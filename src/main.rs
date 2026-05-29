//! Open a shell with a prompt that starts with a dollar sign.
//!

#![deny(clippy::all)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

/// A kind of shell to spawn.
///
/// This determines how to configure the prompt.
/// It is inferred from the program name, or can be overridden by passing a specific shell.
enum Kind {
    Sh,
    Bash,
    Ksh,
    Zsh,
    Fish,
}

/// Support for parsing a shell kind from a string.
/// The string is typically the basename of the shell program, but can be overridden.
impl FromStr for Kind {
    type Err = ();

    /// Parse a shell kind from a string.
    /// # Errors
    /// This will return an error if the string does not match any supported shell kind.
    ///
    /// # Supported values:
    ///
    /// - "sh" => `Kind::Sh`
    /// - "bash" => `Kind::Bash`
    /// - "ksh", "ksh93", "mksh", "pdksh", "loksh" => `Kind::Ksh`
    /// - "zsh" => `Kind::Zsh`
    /// - "fish" => `Kind::Fish`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sh" => Ok(Kind::Sh),
            "bash" => Ok(Kind::Bash),
            "ksh" | "ksh93" | "mksh" | "pdksh" | "loksh" => Ok(Kind::Ksh),
            "zsh" => Ok(Kind::Zsh),
            "fish" => Ok(Kind::Fish),
            _ => Err(()),
        }
    }
}

/// A temporary workspace for shell configuration files.
///
/// This is used in the creation of temporary rc files for bash, ksh, and zsh.
/// The directory is created on demand and automatically cleaned up when the `Workspace` is dropped.
struct Workspace(Option<PathBuf>);

impl Workspace {
    /// Get the temporary directory, creating it if necessary.
    ///
    /// # Panics
    /// This will panic if the directory cannot be created.
    fn dir(&mut self) -> &Path {
        self.0.get_or_insert_with(|| {
            let p = env::temp_dir().join(format!("dollar-{}", std::process::id()));
            fs::create_dir_all(&p).expect("Failed to create temp dir");
            p
        })
    }

    /// Write `body` to `name` inside the directory and return the path.
    ///
    /// # Panics
    /// This will panic if the file cannot be written.
    fn write(&mut self, name: &str, body: &str) -> PathBuf {
        let path = self.dir().join(name);
        fs::write(&path, body).expect("Failed to write temp rc file");
        path
    }
}

/// Automatically clean up the temporary directory.
impl Drop for Workspace {
    /// Remove the temporary directory and all its contents.
    fn drop(&mut self) {
        if let Some(dir) = &self.0 {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn main() {
    // Determine the target shell to spawn.
    // This is either:
    //   1. The first command line argument, if provided.
    //   2. The shell specified by the SHELL environment variable, if set.
    //   3. "sh" as a fallback.
    let target = env::args()
        .nth(1)
        .or_else(|| env::var("SHELL").ok())
        .unwrap_or_else(|| "sh".into());

    // The shell basename is used to determine its kind.
    let base = Path::new(&target)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("sh");

    let (program, kind) = match Kind::from_str(base) {
        Ok(k) => (target, k),
        Err(()) => ("sh".to_string(), Kind::Sh),
    };

    let mut ws = Workspace(None);
    let mut cmd = Command::new(&program);

    // Configure the prompt according to the shell's kind.
    match kind {
        Kind::Sh => {
            // sh supports the PS1 environment variable if ENV is unset.
            cmd.env("PS1", "$ ").env_remove("ENV");
        }
        Kind::Bash => {
            // bash only supports PS1 in interactive shells.
            // --rcfile ignores system-wide bashrc, so we source it.
            // Disable PROMPT_COMMAND too to avoid overwriting PS1.
            let rc = ws.write(
                "bashrc",
                "[ -f ~/.bashrc ] && . ~/.bashrc\nPS1='$ '\nPROMPT_COMMAND=''\n",
            );
            cmd.arg("--rcfile").arg(&rc);
        }
        Kind::Ksh => {
            // ksh is a nigh-zsh mess.
            // PS1, but only if ENV IS set, and ENV is only used if it points to a readable file.
            // So, make a temp env and point ENV to it, and put the PS1 in that file.
            let orig = env::var("ENV").unwrap_or_default();
            let rc = ws.write(
                "kshrc",
                "[ -r \"$_DOLLAR_ENV\" ] && . \"$_DOLLAR_ENV\"\nPS1='$ '\n",
            );
            cmd.env("_DOLLAR_ENV", orig).env("ENV", &rc).arg("-i");
        }
        Kind::Zsh => {
            // zsh uses ZDOTDIR to find its rc files, but only if it's set at startup.
            // Otherwise, it uses ~, and ignores ZDOTDIR even if it's set later.
            // So, make a temp dir, put the rc files there, and set ZDOTDIR to that dir at startup.
            // Also, source the original rc files from the temp ones, so that the shell behaves more like the normal zsh.
            // RPROMPT, and precmd_functions are set to avoid the default right prompt and the timestamp in the prompt.
            let orig = env::var("ZDOTDIR")
                .or_else(|_| env::var("HOME"))
                .unwrap_or_default();
            ws.write(
                ".zshenv",
                "[ -f \"$_DOLLAR_ZDOTDIR/.zshenv\" ] && source \"$_DOLLAR_ZDOTDIR/.zshenv\"\n",
            );
            ws.write(
                ".zshrc",
                "[ -f \"$_DOLLAR_ZDOTDIR/.zshrc\" ] && source \"$_DOLLAR_ZDOTDIR/.zshrc\"\nPROMPT='$ '\nRPROMPT=''\nprecmd_functions=()\n",
            );
            cmd.env("_DOLLAR_ZDOTDIR", orig).env("ZDOTDIR", ws.dir());
        }
        Kind::Fish => {
            // fish doesn't support PS1 at all; but yay! inline commands!
            // We just pass the prompt definitions (in fish, prompt is a fn).
            cmd.arg("-C")
                .arg("function fish_prompt; echo -n '$ '; end; function fish_right_prompt; end");
        }
    }

    // Spawn the shell and wait for it to exit.
    cmd.spawn()
        .expect("Failed to spawn shell")
        .wait()
        .expect("Failed to wait for shell's exit");
}
