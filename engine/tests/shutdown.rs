#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{SECRET, id, start_body};

struct Engine {
    child: Child,
    root: std::path::PathBuf,
    addr: SocketAddr,
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn signal(name: &str, pid: u32) {
    let status = Command::new("kill")
        .args([&format!("-{name}"), &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

fn post_session(addr: SocketAddr, body: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    write!(
        s,
        "POST /v1/sessions HTTP/1.1\r\nhost: engine\r\nauthorization: Bearer {SECRET}\r\n\
         content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

fn start_engine(name: &str) -> (Engine, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("engine-{name}-{}", std::process::id()));
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let pidfile = root.join("ffmpeg.pid");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/fake-ffmpeg/ffmpeg"
    );
    let ffmpeg = bin.join("ffmpeg");
    std::fs::write(
        &ffmpeg,
        format!(
            "#!/bin/sh\ncase \"$*\" in\n*-hls_segment_filename*) echo $$ > {}; exec {fixture} \"$@\" >/dev/null ;;\n*) echo 'ffmpeg version fake'; exit 0 ;;\nesac\n",
            pidfile.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755)).unwrap();

    let conf = root.join("engine.yaml");
    std::fs::write(
        &conf,
        format!(
            "listen: 127.0.0.1:0\nsecret: {SECRET}\ntmp_dir: {}\njobs_dir: {}\nffmpeg_dir: {}\n",
            root.join("live").display(),
            root.join("jobs").display(),
            bin.display()
        ),
    )
    .unwrap();
    let (child, addr) = common::spawn_engine(&conf);
    let engine = Engine { child, root, addr };
    (engine, pidfile)
}

fn running_ffmpeg(engine: &Engine, pidfile: &Path) -> u32 {
    let input = engine.root.join("in.mkv");
    std::fs::write(&input, b"").unwrap();
    let body = start_body(&id(1), &input, "hevc", "mp4").to_string();
    let reply = post_session(engine.addr, &body);
    assert!(reply.starts_with("HTTP/1.1 2"), "{reply}");
    assert!(
        until(Duration::from_secs(5), || pidfile.exists()),
        "ffmpeg never started"
    );
    let pid: u32 = std::fs::read_to_string(pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(pid));
    pid
}

#[test]
fn sigterm_kills_ffmpeg_and_exits() {
    let (mut engine, pidfile) = start_engine("sigterm");
    let ffmpeg = running_ffmpeg(&engine, &pidfile);

    signal("TERM", engine.child.id());

    assert!(
        until(Duration::from_secs(3), || !alive(ffmpeg)),
        "ffmpeg {ffmpeg} survived SIGTERM"
    );
    assert!(
        until(Duration::from_secs(3), || engine
            .child
            .try_wait()
            .unwrap()
            .is_some()),
        "engine did not exit"
    );
}

#[test]
fn stuck_connection_does_not_block_shutdown() {
    let (mut engine, pidfile) = start_engine("stuck");
    running_ffmpeg(&engine, &pidfile);
    let mut idle = TcpStream::connect(engine.addr).unwrap();
    write!(idle, "GET /v1/health HTTP/1.1\r\nhost: engine\r\n\r\n").unwrap();
    let mut slow = TcpStream::connect(engine.addr).unwrap();
    write!(
        slow,
        "POST /v1/sessions HTTP/1.1\r\nhost: engine\r\ncontent-length: 10\r\n\r\n"
    )
    .unwrap();

    signal("TERM", engine.child.id());

    assert!(
        until(Duration::from_secs(15), || engine
            .child
            .try_wait()
            .unwrap()
            .is_some()),
        "engine hung on an open connection"
    );
}

#[test]
fn second_signal_exits_immediately() {
    let (mut engine, pidfile) = start_engine("second");
    running_ffmpeg(&engine, &pidfile);
    let mut slow = TcpStream::connect(engine.addr).unwrap();
    write!(
        slow,
        "POST /v1/sessions HTTP/1.1\r\nhost: engine\r\ncontent-length: 10\r\n\r\n"
    )
    .unwrap();

    signal("TERM", engine.child.id());
    std::thread::sleep(Duration::from_millis(300));
    signal("TERM", engine.child.id());

    assert!(
        until(Duration::from_secs(2), || engine
            .child
            .try_wait()
            .unwrap()
            .is_some()),
        "second signal did not exit"
    );
}
