# Source this file, then run: rtl-wrap codex gemini
# Does not edit your profile or enable any wrappers until explicitly called.
rtl-wrap() {
  local agent
  for agent in "$@"; do
    if [[ ! "$agent" =~ '^[A-Za-z0-9_-]+$' || "$agent" == rtl || "$agent" == rtl-wrap ]]; then
      print -u2 -- "rtl-wrap: invalid command name: $agent"
      return 1
    fi
    if (( $+functions[$agent] || $+aliases[$agent] )); then
      print -u2 -- "rtl-wrap: $agent is already a function or alias; leaving it unchanged"
      return 1
    fi
    if (( ! $+commands[$agent] || ! $+commands[rtl] )); then
      print -u2 -- "rtl-wrap: both rtl and $agent must be installed and on PATH"
      return 1
    fi
    functions[$agent]='
      if [[ -n ${RTL_ACTIVE:-} ]]; then
        command "${funcstack[1]}" "$@"
      else
        command rtl "${funcstack[1]}" "$@"
      fi
    '
  done
}

