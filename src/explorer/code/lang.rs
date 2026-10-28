//! Which highlighter a file gets: by its name, else by its `#!` line.

/// The highlighter for a file, by its name. Configs without a grammar of their own get the
/// nearest one: shell for nginx/systemd/env/dotfiles (comments, strings, `$vars`), TOML for
/// INI-style files (`[section]`, `key = value`). SQL's grammar (tree-sitter-sequel) needs
/// `cc` 1.2.x: Cargo.lock keeps it there (`cargo update -p cc --precise 1.2.67`).
pub fn language_for(path: &str) -> &'static str {
    let p = std::path::Path::new(path);
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    match name.as_str() {
        "dockerfile" | "containerfile" | "procfile" | "crontab" | ".env" | ".bashrc" | ".zshrc"
        | ".profile" | ".bash_profile" | ".bash_aliases" | ".gitignore" | ".dockerignore"
        | ".htaccess" | "nginx.conf" => return "bash",
        "makefile" | "gnumakefile" => return "make",
        "cmakelists.txt" => return "cmake",
        "gemfile" | "rakefile" | "vagrantfile" | "podfile" | "brewfile" => return "ruby",
        "cargo.lock" | "pipfile" => return "toml",
        ".babelrc" | ".eslintrc" | ".prettierrc" | "composer.lock" => return "json",
        _ => {}
    }
    if name.starts_with("dockerfile.") || name.starts_with(".env.") {
        return "bash";
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" | "pyi" | "pyw" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "json" | "jsonc" | "json5" | "geojson" | "webmanifest" | "ipynb" => "json",
        "yaml" | "yml" => "yaml",
        "toml" | "ini" | "cfg" | "cnf" | "properties" | "editorconfig" => "toml",
        "md" | "markdown" | "mdx" => "markdown",
        "html" | "htm" | "xhtml" | "xml" | "svg" | "plist" | "xsl" | "vue" => "html",
        "css" | "scss" | "sass" | "less" => "css",
        "sh" | "bash" | "zsh" | "ksh" | "fish" | "conf" | "env" | "service" | "timer"
        | "socket" | "mount" | "target" | "nginx" | "vhost" | "htaccess" => "bash",
        "go" => "go",
        "php" | "phtml" | "inc" => "php",
        "rb" | "rake" | "gemspec" => "ruby",
        "erb" => "erb",
        "ejs" => "ejs",
        "swift" => "swift",
        "java" | "groovy" | "gradle" => "java",
        "kt" | "kts" => "kotlin",
        "scala" | "sc" => "scala",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "ino" => "cpp",
        "cs" => "csharp",
        "zig" => "zig",
        "lua" => "lua",
        "sql" | "psql" | "mysql" => "sql",
        "mk" | "mak" => "make",
        "cmake" => "cmake",
        // A patch as plain text: the bundled diff query paints deletions with the keyword's
        // colour and additions with the string's — colours that say nothing about a change. The
        // gutter's marks (`git.rs`) are what say it, in the theme's own success and danger.
        "diff" | "patch" => "text",
        "graphql" | "gql" => "graphql",
        "proto" => "proto",
        "ex" | "exs" => "elixir",
        "svelte" => "svelte",
        "astro" => "astro",
        _ => "text",
    }
}

/// For a file the name says nothing about: its `#!` line.
pub fn language_for_text(path: &str, text: &str) -> &'static str {
    let by_name = language_for(path);
    if by_name != "text" {
        return by_name;
    }
    let first = text.lines().next().unwrap_or("");
    let Some(shebang) = first.strip_prefix("#!") else {
        return "text";
    };
    let interp = shebang
        .split_whitespace()
        .find(|w| !w.ends_with("/env") && !w.starts_with('-'))
        .and_then(|w| w.rsplit('/').next())
        .unwrap_or("");
    match interp {
        i if i.starts_with("python") => "python",
        "node" | "deno" | "bun" => "javascript",
        "ruby" => "ruby",
        "php" => "php",
        "lua" => "lua",
        "sh" | "bash" | "zsh" | "ksh" | "dash" | "fish" => "bash",
        _ => "text",
    }
}

#[cfg(test)]
mod language_tests {
    use super::{language_for, language_for_text};

    #[test]
    fn by_name_and_shebang() {
        for (path, lang) in [
            ("/etc/nginx/conf.d/site.conf", "bash"),
            ("/srv/app/.env.production", "bash"),
            ("/srv/app/Dockerfile", "bash"),
            ("/etc/mysql/my.cnf", "toml"),
            ("/etc/php/php.ini", "toml"),
            ("/srv/app/Makefile", "make"),
            ("/srv/app/src/App.jsx", "javascript"),
            ("/srv/app/styles/main.scss", "css"),
            ("/srv/app/config.xml", "html"),
            ("/srv/app/docker-compose.yml", "yaml"),
            ("/srv/app/index.php", "php"),
            ("/srv/app/schema.sql", "sql"),
            ("/srv/app/notes.txt", "text"),
        ] {
            assert_eq!(language_for(path), lang, "{path}");
        }
        assert_eq!(
            language_for_text("/usr/local/bin/deploy", "#!/usr/bin/env python3\n"),
            "python"
        );
        assert_eq!(
            language_for_text("/usr/local/bin/backup", "#!/bin/bash -e\n"),
            "bash"
        );
        assert_eq!(language_for_text("/usr/local/bin/x", "hello"), "text");
    }
}
