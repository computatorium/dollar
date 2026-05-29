use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

enum Kind {
    Sh,
    Bash,
    Ksh,
    Zsh,
    Fish,
}

impl FromStr for Kind {
    type Err = ();

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

struct Workspace(Option<PathBuf>);

impl Workspace {
    fn dir(&mut self) -> PathBuf {
        self.0
            .get_or_insert_with(|| {
                let p = env::temp_dir().join(format!("dollar-{}", std::process::id()));
                fs::create_dir_all(&p).expect("Failed to create temp dir");
                p
            })
            .clone()
    }

    fn write(&mut self, name: &str, body: &str) -> PathBuf {
        let path = self.dir().join(name);
        fs::write(&path, body).expect("Failed to write temp rc file");
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = fs::remove_dir_all(p);
        }
    }
}

fn main() {
    let target = env::args()
        .nth(1)
        .or_else(|| env::var("SHELL").ok())
        .unwrap_or_else(|| "sh".into());

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

    match kind {
        Kind::Sh => {
            cmd.env("PS1", "$ ").env_remove("ENV");
        }
        Kind::Bash => {
            let rc = ws.write(
                "bashrc",
                "[ -f ~/.bashrc ] && . ~/.bashrc\nPS1='$ '\nPROMPT_COMMAND=''\n",
            );
            cmd.arg("--rcfile").arg(&rc);
        }
        Kind::Ksh => {
            let orig = env::var("ENV").unwrap_or_default();
            let rc = ws.write(
                "kshrc",
                "[ -r \"$_DOLLAR_ENV\" ] && . \"$_DOLLAR_ENV\"\nPS1='$ '\n",
            );
            cmd.env("_DOLLAR_ENV", orig).env("ENV", &rc).arg("-i");
        }
        Kind::Zsh => {
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
            cmd.arg("-C")
                .arg("function fish_prompt; echo -n '$ '; end; function fish_right_prompt; end");
        }
    }

    cmd.spawn()
        .expect("Failed to spawn shell")
        .wait()
        .expect("Failed to wait for shell's exit");
}
