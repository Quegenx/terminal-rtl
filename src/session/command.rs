use std::ffi::OsString;

use anyhow::{Context, Result};
use portable_pty::CommandBuilder;

pub(super) fn child_command(args: &[OsString]) -> Result<CommandBuilder> {
    let mut cmd = platform_command(args);
    // portable-pty defaults to the user's home, not the caller's directory.
    cmd.cwd(std::env::current_dir().context("cannot resolve working directory")?);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("RTL_ACTIVE", "1");
    Ok(cmd)
}

#[cfg(not(windows))]
fn platform_command(args: &[OsString]) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(&args[0]);
    cmd.args(&args[1..]);
    cmd
}

#[cfg(windows)]
fn platform_command(args: &[OsString]) -> CommandBuilder {
    use base64::Engine as _;
    // ConPTY/CreateProcess cannot execute npm .cmd shims directly. Resolve the
    // executable ourselves and use a quoted, encoded PowerShell script only for
    // batch files. Native executables keep direct argument passing.
    let path = resolve_windows_command(&args[0]);
    let batch = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !batch {
        let mut cmd = CommandBuilder::new(path);
        cmd.args(&args[1..]);
        return cmd;
    }
    let script = powershell_batch_script(path.as_os_str(), &args[1..]);
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.args(["-NoLogo", "-NoProfile", "-EncodedCommand"]);
    cmd.arg(base64::engine::general_purpose::STANDARD.encode(bytes));
    cmd
}

#[cfg(any(windows, test))]
fn powershell_batch_script(path: &std::ffi::OsStr, args: &[OsString]) -> String {
    let quoted = |s: &std::ffi::OsStr| format!("'{}'", s.to_string_lossy().replace('\'', "''"));
    let mut script = format!(
        "$ErrorActionPreference = 'Stop'; [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; & {}",
        quoted(path)
    );
    for arg in args {
        script.push(' ');
        script.push_str(&quoted(arg));
    }
    script.push_str("; if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE } else { exit 1 }");
    script
}

#[cfg(windows)]
fn resolve_windows_command(name: &std::ffi::OsStr) -> std::path::PathBuf {
    use std::path::PathBuf;
    let candidate = PathBuf::from(name);
    let mut dirs = vec![PathBuf::new()];
    if candidate.components().count() == 1
        && let Some(path) = std::env::var_os("PATH")
    {
        dirs.extend(std::env::split_paths(&path));
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    resolve_windows_in(&candidate, &dirs, &extensions)
}

#[cfg(any(windows, test))]
fn resolve_windows_in(
    candidate: &std::path::Path,
    dirs: &[std::path::PathBuf],
    extensions: &str,
) -> std::path::PathBuf {
    use std::path::PathBuf;
    for dir in dirs {
        let path = dir.join(candidate);
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(supported_windows_extension)
        {
            return path;
        }
        if path.extension().is_none() {
            for ext in extensions
                .split(';')
                .filter(|e| supported_windows_extension(e.trim_start_matches('.')))
            {
                let mut full = path.as_os_str().to_os_string();
                full.push(ext);
                let full = PathBuf::from(full);
                if full.is_file() {
                    return full;
                }
            }
        }
    }
    candidate.to_path_buf()
}

#[cfg(any(windows, test))]
fn supported_windows_extension(ext: &str) -> bool {
    ["com", "exe", "bat", "cmd"]
        .iter()
        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_resolution_ignores_shell_shims_and_obeys_pathext() {
        let directory = std::env::temp_dir().join(format!("rtl-resolver-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let result = std::panic::catch_unwind(|| {
            for name in ["agent", "agent.CMD", "agent.EXE", "agent.ps1"] {
                std::fs::write(directory.join(name), b"fixture").unwrap();
            }
            let dirs = std::slice::from_ref(&directory);
            assert_eq!(
                resolve_windows_in(std::path::Path::new("agent"), dirs, ".CMD;.EXE;.PS1"),
                directory.join("agent.CMD")
            );
            assert_eq!(
                resolve_windows_in(std::path::Path::new("agent"), dirs, ".EXE;.CMD"),
                directory.join("agent.EXE")
            );
            assert_eq!(
                resolve_windows_in(std::path::Path::new("agent.EXE"), dirs, ".CMD;.EXE"),
                directory.join("agent.EXE")
            );
        });
        std::fs::remove_dir_all(directory).unwrap();
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }
    #[test]
    fn windows_batch_arguments_use_literal_powershell_strings() {
        let args: Vec<OsString> = include_str!("../../tests/fixtures/windows-arguments.txt")
            .lines()
            .map(Into::into)
            .collect();
        let script =
            powershell_batch_script(std::ffi::OsStr::new("C:\\space folder\\O'Brien.cmd"), &args);
        assert!(script.contains(
            "'space value' '' 'O''Brien' 'שלום' 'a&b' 'a|b' 'a>b' '%PATH%' '$HOME' '(parentheses)'"
        ));
    }
}
