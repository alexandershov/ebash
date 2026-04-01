use anyhow::{anyhow, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use rstest::rstest;
use rstest_reuse::{self, *};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

#[template]
#[rstest]
#[case::zsh("/bin/zsh")]
#[case::bash("/bin/bash")]
fn test_met_ondersteunde_shells(#[case] shell: &str) {}

/// test command mode: `ebash -c "count number of python files"`
#[apply(test_met_ondersteunde_shells)]
fn test_opdrachtmodus(shell: &str) -> Result<()> {
    let output = voer_ebash_uit(
        "default",
        &vec!["-c".into(), "count number of python files".into()],
        &omgeving_met_shell(shell),
        Stdio::piped(),
    )?;
    // expect 2 python files:
    // * fixtures/default/first.py
    // * fixtures/default/second.py
    assert_eq!(output.trim(), "2");
    Ok(())
}

/// test script file mode: `ebash third.sh`
#[apply(test_met_ondersteunde_shells)]
fn test_script_bestandsmodus(shell: &str) -> Result<()> {
    let output = voer_ebash_uit(
        "default",
        &vec!["third.sh".into()],
        &omgeving_met_shell(shell),
        Stdio::piped(),
    )?;
    // third.sh contains "count number of shell scripts"
    // expect 1 shell script:
    // * fixtures/default/third.sh
    assert_eq!(output.trim(), "1");
    Ok(())
}

/// test script stdin mode: `ebash < third.sh`
#[apply(test_met_ondersteunde_shells)]
fn test_script_stdin_modus(shell: &str) -> Result<()> {
    let fixture_dir = verkrijg_fixture_map("default")?;
    // script_path == fixtures/default/third.sh
    let script_path = fixture_dir.join("third.sh");
    let input = File::open(script_path)?;

    let output = voer_ebash_uit("default", &vec![], &omgeving_met_shell(shell), input.into())?;
    // third.sh contains "count number of shell scripts"
    // expect 1 shell script:
    // * fixtures/default/third.sh
    assert_eq!(output.trim(), "1");
    Ok(())
}

// test interactive mode: `ebash` with pseudo terminal
#[apply(test_met_ondersteunde_shells)]
// test that interactive mode works when SHELL=/bin/ebash
#[case::ebash("/bin/ebash")]
fn test_interactieve_modus(shell: &str) -> Result<()> {
    let output = voer_ebash_interactief_uit(
        "default",
        &vec!["show names of python files".into()],
        &omgeving_met_shell(shell),
    )?;
    assert!(output.contains("first.py"));
    assert!(output.contains("second.py"));
    assert!(!output.contains("third.sh"));
    Ok(())
}

// test interactive mode history: `ebash` with pseudo terminal and references to previous command
// to check that history is working
#[apply(test_met_ondersteunde_shells)]
fn test_interactieve_modus_geschiedenis(shell: &str) -> Result<()> {
    let output = voer_ebash_interactief_uit(
        "default",
        &vec![
            "show names of python files".into(),
            "show their names in capital letters".into(),
        ],
        &omgeving_met_shell(shell),
    )?;
    assert!(output.contains("FIRST.PY"));
    assert!(output.contains("SECOND.PY"));
    assert!(!output.contains("THIRD.SH"));
    Ok(())
}

/// Return {"SHELL": shell} that is used to pass environment to ebash.
fn omgeving_met_shell(shell: &str) -> HashMap<OsString, OsString> {
    let mut envs = HashMap::new();
    envs.insert("SHELL".into(), shell.into());
    envs
}

/// Run ebash with the given args and environment variables
/// Return stdout as a string.
fn voer_ebash_uit(
    fixture_name: &str,
    args: &[OsString],
    envs: &HashMap<OsString, OsString>,
    stdin: Stdio,
) -> Result<String> {
    let xdg = TempDir::new()?;
    fs::create_dir(xdg.path().join("ebash"))?;
    // config with model = "gpt-4.1"
    fs::write(
        xdg.path().join("ebash/ebash.toml"),
        "config = \"CgdncHQtNC4x\"",
    )?;

    let effective_envs = bouw_effectieve_omgevingen(
        envs,
        xdg.path(),
        &std::env::var_os("OPENAI_API_KEY").ok_or(anyhow!("OPENAI_API_KEY is not set"))?,
    );

    let fixture_dir = verkrijg_fixture_map(fixture_name)?;
    // CARGO_BIN_EXE_ebash holds the absolute path to a binary target’s executable
    let mut command = Command::new(env!("CARGO_BIN_EXE_ebash"));
    let output = command
        .args(args)
        .envs(effective_envs)
        .stdin(stdin)
        .current_dir(fixture_dir)
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    if !output.status.success() {
        return Err(anyhow!(
            "command {command:?} exited with {}\nstdout = \n{stdout}\nstderr = \n{stderr}",
            output.status,
        ));
    }
    Ok(stdout)
}

/// Run ebash in interactive mode
fn voer_ebash_interactief_uit(
    fixture_name: &str,
    commands: &[String],
    envs: &HashMap<OsString, OsString>,
) -> Result<String> {
    let xdg = TempDir::new()?;

    let effective_envs = bouw_effectieve_omgevingen(
        envs,
        xdg.path(),
        &std::env::var_os("OPENAI_API_KEY").ok_or(anyhow!("OPENAI_API_KEY is not set"))?,
    );
    let mut effective_commands = Vec::from(commands);
    // add `exit` command so we always exit the interactive shell
    effective_commands.push("exit".into());

    // fixture dir is {file!()}/../fixtures/{fixture_name}
    let fixture_dir = verkrijg_fixture_map(fixture_name)?;

    // CARGO_BIN_EXE_ebash holds the absolute path to a binary target’s executable
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_ebash"));

    // portable_pty::CommandBuilder doesn't support .envs method, so we need to add each envvar one by one
    for (key, val) in effective_envs {
        command.env(key, val)
    }
    command.cwd(fixture_dir);

    // create pty & master/slave pair
    let pty_system = native_pty_system();

    let pty_pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // command.clone() is required because otherwise we'll get "use after move"
    // in `command.as_unix_command_line()`
    let mut child = pty_pair.slave.spawn_command(command.clone())?;
    // we don't need a slave side of the pty_pair after spawning a child
    // without this drop later `reader.read_to_string(&mut output)?` hangs on linux
    drop(pty_pair.slave);

    let mut reader = pty_pair.master.try_clone_reader()?;

    // We write to master. Master writes are available as reads in slave.
    // Child will write to slave. Slave writes are available as reads in master.
    let mut writer = pty_pair.master.take_writer()?;
    for cmd in effective_commands {
        // \r is required for zsh to apply ^M binding when commands are sent one after another
        // without waiting
        writeln!(writer, "{cmd}\r")?;
    }

    // read from master before waiting for a child to avoid deadlock
    // if we swap the order and child overflows tty buffer, then it'll wait indefinitely because
    // no one reads from master side of the tty
    let mut output = String::new();
    reader.read_to_string(&mut output)?;

    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!(
            "command `{}` exited with code {}, output was\n{output}",
            command.as_unix_command_line()?,
            status.exit_code(),
        ));
    }
    Ok(output)
}

/// Build an isolated environment used to run ebash in tests
/// ebash will search for configs in the `xdg` directory
fn bouw_effectieve_omgevingen(
    envs: &HashMap<OsString, OsString>,
    xdg: &Path,
    openai_api_key: &OsString,
) -> HashMap<OsString, OsString> {
    let mut effective_envs = envs.clone();
    effective_envs.insert("XDG_STATE_HOME".into(), xdg.into());
    effective_envs.insert("XDG_CONFIG_HOME".into(), xdg.into());
    effective_envs.insert("OPENAI_API_KEY".into(), openai_api_key.into());
    effective_envs
}

// E.g. `default -> ../fixtures/default`
fn verkrijg_fixture_map(fixture_name: &str) -> Result<PathBuf> {
    let file = PathBuf::from(file!());
    let parent = file
        .parent()
        .ok_or(anyhow!("can't get dir for fixture `{}`", fixture_name))?;
    Ok(parent.join("fixtures").join(fixture_name))
}
