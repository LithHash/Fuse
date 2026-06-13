pub fn parse_roblox_path(text: &str) -> Vec<String> {
    let mut path = Vec::new();
    for segment in text.split('/') {
        if !segment.is_empty() {
            path.push(segment.to_string());
        }
    }

    if !path.is_empty() && (path[0] == "StarterPlayerScripts" || path[0] == "StarterCharacterScripts")
    {
        path.insert(0, "StarterPlayer".to_string());
    }

    path
}

pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();

    let mut p = 0;
    let mut t = 0;
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
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

    #[test]
    fn glob_matching() {
        assert!(glob_match("server", "server"));
        assert!(!glob_match("server", "server.luau"));
        assert!(glob_match("*.server.luau", "DamageService.server.luau"));
        assert!(!glob_match("*.server.luau", "DamageService.client.luau"));
        assert!(glob_match("?at", "cat"));
        assert!(!glob_match("?at", "cart"));
        assert!(glob_match("a*b*c", "axxbyyc"));
    }

    #[test]
    fn roblox_path_parsing() {
        assert_eq!(parse_roblox_path("ReplicatedStorage"), ["ReplicatedStorage"]);
        assert_eq!(
            parse_roblox_path("ReplicatedStorage/Source"),
            ["ReplicatedStorage", "Source"]
        );
        assert_eq!(
            parse_roblox_path("StarterPlayerScripts"),
            ["StarterPlayer", "StarterPlayerScripts"]
        );
    }
}
