use std::error::Error;
use std::process::Command;

/// Runs `git describe` on the repo to get the development version.
fn git_describe() -> Result<Option<String>, Box<dyn Error>> {
    match Command::new("git")
        .current_dir(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .args(["describe", "--tags", "--dirty"])
        .output()
    {
        Ok(res) => {
            let status = &res.status;
            if status.success() {
                let res = String::from_utf8_lossy(&res.stdout).into_owned();
                Ok(Some(res.trim().to_string()))
            } else {
                println!("cargo:warning=Executing `git describe` failed {status:?}");
                Ok(None)
            }
        }
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound) => {
            println!("cargo:warning=Could not find `git` command");
            Ok(None)
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute `git` command");
            eprintln!("{e}");
            Ok(None)
        }
    }
}

fn git_version() -> Result<Option<String>, Box<dyn Error>> {
    let Some(describe) = git_describe()? else {
        return Ok(None);
    };
    // mirrors the style of https://github.com/choffmeister/git-describe-semver
    // Does not currently support `-rc`, `-alpha`, and friends
    use regex_lite::Regex;
    const SIMPLE_VERSION_PAT: &str = r#"v?\d+\.\d+\.\d+"#;
    let simple_version = Regex::new(&format!("^{SIMPLE_VERSION_PAT}$")).unwrap();
    let complex_version = Regex::new(&format!(
        r#"(?x)^
            (?<primary>{SIMPLE_VERSION_PAT})
            -
            (?<build_num>\d+)
            -g
            (?<rev>[0-9A-Fa-f]+)
            (?<dirty>-dirty)?
            $
        "#
    ))
    .unwrap();
    if simple_version.is_match(&describe) {
        Ok(Some(describe))
    } else if let Some(mat) = complex_version.captures(&describe) {
        let mut res = format!(
            "{primary}-dev.{build_num}.g{rev}",
            primary = &mat["primary"],
            build_num = &mat["build_num"],
            rev = &mat["rev"],
        );
        if mat.name("dirty").is_some() {
            res.push_str("+dirty");
        }
        Ok(Some(res))
    } else {
        panic!("Failed to parse output of `git describe`: {describe:?}")
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(version) = git_version()? {
        println!("cargo:rustc-env=GIT_VERSION={version}");
    }
    Ok(())
}
