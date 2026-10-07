use crate::mapper_config::GameModeProcessMatcher;

pub(super) fn matches(matcher: &GameModeProcessMatcher, candidate: &str) -> bool {
    let needle = matcher.name.trim().to_lowercase();
    let candidate = candidate.trim().to_lowercase();
    if needle.is_empty() || candidate.is_empty() {
        return false;
    }
    if needle.contains('*') || needle.contains('?') {
        wildcard_matches(&needle, &candidate)
    } else {
        candidate == needle
            || candidate.strip_suffix(".exe") == Some(needle.as_str())
            || needle.strip_suffix(".exe") == Some(candidate.as_str())
    }
}

fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let candidate: Vec<char> = candidate.chars().collect();
    let (mut p, mut c) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while c < candidate.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == candidate[c]) {
            p += 1;
            c += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            retry = c;
        } else if let Some(index) = star {
            p = index + 1;
            retry += 1;
            c = retry;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(name: &str) -> GameModeProcessMatcher {
        GameModeProcessMatcher {
            name: name.into(),
            only_active_window: false,
            is_blacklist: false,
        }
    }

    #[test]
    fn exact_names_do_not_match_unrelated_processes() {
        assert!(matches(&matcher(" GAME "), "game"));
        assert!(matches(&matcher("game"), "Game.exe"));
        assert!(!matches(&matcher("game"), "game-launcher"));
        assert!(!matches(&matcher("steam"), "steamwebhelper"));
        assert!(!matches(&matcher(""), "game"));
    }

    #[test]
    fn explicit_wildcards_match_the_entire_name() {
        assert!(matches(&matcher("steam*"), "steamwebhelper"));
        assert!(matches(&matcher("game?.exe"), "game1.exe"));
        assert!(!matches(&matcher("game?.exe"), "game12.exe"));
        assert!(matches(&matcher("*game*"), "my-game-launcher"));
        assert!(matches(&matcher("*игра?"), "моя-игра1"));
        assert!(!matches(&matcher("game*"), "other-game"));
        assert!(matches(&matcher("*a*b"), "zaaab"));
    }
}
