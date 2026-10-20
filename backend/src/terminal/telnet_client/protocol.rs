//! Telnet without the I/O: option negotiation (RFC 854/855, with RFC 1143's
//! rule of never answering an answer), and the login prompts a stored
//! password is typed into.

pub const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

pub const BINARY: u8 = 0;
pub const ECHO: u8 = 1;
pub const SGA: u8 = 3;
pub const TTYPE: u8 = 24;
pub const NAWS: u8 = 31;

const TERMINAL_TYPE: &[u8] = b"XTERM-256COLOR";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Opt {
    No,
    /// Asked for; the other side's answer settles it without a reply.
    Asked,
    Yes,
}

#[derive(Clone, Copy)]
enum Parse {
    Data,
    Iac,
    Verb(u8),
    Sub,
    SubIac,
}

pub struct Telnet {
    /// What we do (WILL/WONT), and what the server does (DO/DONT).
    us: [Opt; 256],
    him: [Opt; 256],
    parse: Parse,
    sub: Vec<u8>,
    last_cr: bool,
    pub size: (u16, u16),
}

impl Telnet {
    pub fn new(size: (u16, u16)) -> Self {
        Telnet { us: [Opt::No; 256], him: [Opt::No; 256], parse: Parse::Data, sub: Vec::new(), last_cr: false, size }
    }

    /// What a client says first, as PuTTY does: it will report its window
    /// size and terminal type, and would like the server to echo.
    pub fn hello(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for opt in [NAWS, TTYPE, SGA] {
            self.us[opt as usize] = Opt::Asked;
            out.extend([IAC, WILL, opt]);
        }
        for opt in [ECHO, SGA] {
            self.him[opt as usize] = Opt::Asked;
            out.extend([IAC, DO, opt]);
        }
        out
    }

    /// The server echoes what is typed; until it does, the client must.
    pub fn server_echoes(&self) -> bool {
        self.him[ECHO as usize] == Opt::Yes
    }

    /// Bytes from the server: what goes to the screen, and what to answer.
    pub fn feed(&mut self, bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let (mut screen, mut reply) = (Vec::new(), Vec::new());
        for &b in bytes {
            self.parse = match (self.parse, b) {
                (Parse::Data, IAC) => Parse::Iac,
                (Parse::Data, _) => {
                    // A bare CR travels as CR NUL; the NUL is not data.
                    if !(self.last_cr && b == 0) {
                        screen.push(b);
                    }
                    self.last_cr = b == b'\r';
                    Parse::Data
                }
                (Parse::Iac, IAC) => {
                    screen.push(IAC);
                    Parse::Data
                }
                (Parse::Iac, DO | DONT | WILL | WONT) => Parse::Verb(b),
                (Parse::Iac, SB) => {
                    self.sub.clear();
                    Parse::Sub
                }
                (Parse::Iac, _) => Parse::Data,
                (Parse::Verb(verb), opt) => {
                    self.negotiate(verb, opt, &mut reply);
                    Parse::Data
                }
                (Parse::Sub, IAC) => Parse::SubIac,
                (Parse::Sub, _) => {
                    if self.sub.len() < 1024 {
                        self.sub.push(b);
                    }
                    Parse::Sub
                }
                (Parse::SubIac, SE) => {
                    self.subnegotiation(&mut reply);
                    Parse::Data
                }
                (Parse::SubIac, _) => {
                    self.sub.push(b);
                    Parse::Sub
                }
            };
        }
        (screen, reply)
    }

    fn negotiate(&mut self, verb: u8, opt: u8, reply: &mut Vec<u8>) {
        let i = opt as usize;
        match verb {
            DO => match self.us[i] {
                Opt::Yes => {}
                state => {
                    let ours = matches!(opt, NAWS | TTYPE | SGA | BINARY);
                    if !ours {
                        reply.extend([IAC, WONT, opt]);
                        return;
                    }
                    if state == Opt::No {
                        reply.extend([IAC, WILL, opt]);
                    }
                    self.us[i] = Opt::Yes;
                    if opt == NAWS {
                        reply.extend(self.window_size());
                    }
                }
            },
            DONT => {
                if self.us[i] == Opt::Yes {
                    reply.extend([IAC, WONT, opt]);
                }
                self.us[i] = Opt::No;
            }
            WILL => match self.him[i] {
                Opt::Yes => {}
                state => {
                    if !matches!(opt, ECHO | SGA | BINARY) {
                        reply.extend([IAC, DONT, opt]);
                        return;
                    }
                    if state == Opt::No {
                        reply.extend([IAC, DO, opt]);
                    }
                    self.him[i] = Opt::Yes;
                }
            },
            _ => {
                if self.him[i] == Opt::Yes {
                    reply.extend([IAC, DONT, opt]);
                }
                self.him[i] = Opt::No;
            }
        }
    }

    fn subnegotiation(&mut self, reply: &mut Vec<u8>) {
        // TTYPE SEND: answer IS and the type.
        if self.sub.first() == Some(&TTYPE) && self.sub.get(1) == Some(&1) {
            reply.extend([IAC, SB, TTYPE, 0]);
            reply.extend(TERMINAL_TYPE);
            reply.extend([IAC, SE]);
        }
    }

    /// NAWS with the current size, once the server has agreed to it.
    pub fn window_size(&self) -> Vec<u8> {
        if self.us[NAWS as usize] != Opt::Yes {
            return Vec::new();
        }
        let mut out = vec![IAC, SB, NAWS];
        for half in [self.size.0.to_be_bytes(), self.size.1.to_be_bytes()].concat() {
            out.push(half);
            if half == IAC {
                out.push(IAC);
            }
        }
        out.extend([IAC, SE]);
        out
    }

    /// Typed bytes as they go on the wire: IAC doubled, and Enter's CR sent
    /// as CR NUL unless both sides agreed on binary.
    pub fn encode(&self, typed: &[u8]) -> Vec<u8> {
        let binary = self.us[BINARY as usize] == Opt::Yes;
        let mut out = Vec::with_capacity(typed.len() + 4);
        for &b in typed {
            out.push(b);
            match b {
                IAC => out.push(IAC),
                b'\r' if !binary => out.push(0),
                _ => {}
            }
        }
        out
    }
}

/// Answers the server's login prompt with the saved user and its password
/// prompt with the stored password, once each, then gets out of the way. With
/// no stored password the operator types it. A second prompt means the login
/// was refused, and the operator takes over.
pub struct AutoLogin {
    user: String,
    password: Option<String>,
    sent_user: bool,
    sent_password: bool,
    done: bool,
    tail: String,
}

impl AutoLogin {
    pub fn new(user: String, password: Option<String>) -> Self {
        AutoLogin { user, password, sent_user: false, sent_password: false, done: false, tail: String::new() }
    }

    pub fn stop(&mut self) {
        self.done = true;
    }

    /// Screen output; returns a line to type, with its Enter, if a prompt
    /// has just been shown.
    pub fn seen(&mut self, screen: &[u8]) -> Option<String> {
        if self.done {
            return None;
        }
        let mut escape = false;
        for &b in screen {
            // Colour and cursor sequences are not part of a prompt.
            match (escape, b) {
                (false, 0x1b) => escape = true,
                (true, b'a'..=b'z' | b'A'..=b'Z') => escape = false,
                (true, _) => {}
                (false, b'\r' | b'\n') => self.tail.clear(),
                (false, _) => self.tail.push((b as char).to_ascii_lowercase()),
            }
        }
        if self.tail.len() > 200 {
            self.tail = self.tail.split_off(self.tail.len() - 100);
        }
        let prompt = self.tail.trim_end().to_string();
        let asks_user = ["login:", "username:", "user name:", "user:"].iter().any(|p| prompt.ends_with(p));
        if prompt.ends_with("password:") {
            self.tail.clear();
            if self.sent_password {
                self.done = true;
                return None;
            }
            self.sent_password = true;
            if self.password.is_none() {
                self.done = true;
            }
            return self.password.clone().map(|p| format!("{p}\r"));
        }
        if asks_user && !self.sent_user {
            self.tail.clear();
            self.sent_user = true;
            return Some(format!("{}\r", self.user));
        }
        if asks_user && self.sent_password {
            // Asked to log in again: the password was not accepted.
            self.done = true;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_accepts_what_it_supports_and_never_answers_an_answer() {
        let mut t = Telnet::new((120, 40));
        let hello = t.hello();
        assert!(hello.windows(3).any(|w| w == [IAC, WILL, NAWS]));
        // The server agrees to NAWS (no WILL again, just the size), echoes,
        // and asks for something unknown.
        let (screen, reply) = t.feed(&[IAC, DO, NAWS, IAC, WILL, ECHO, IAC, DO, 39, b'h', b'i']);
        assert_eq!(screen, b"hi");
        assert_eq!(reply, [IAC, SB, NAWS, 0, 120, 0, 40, IAC, SE, IAC, WONT, 39]);
        assert!(t.server_echoes());
        // Repeating itself gets no answer, which is what stops loops.
        assert!(t.feed(&[IAC, WILL, ECHO, IAC, DO, NAWS]).1.is_empty());
        assert_eq!(t.feed(&[IAC, WILL, 200]).1, [IAC, DONT, 200]);
    }

    #[test]
    fn terminal_type_is_reported_and_data_is_unescaped() {
        let mut t = Telnet::new((80, 24));
        let (_, reply) = t.feed(&[IAC, DO, TTYPE, IAC, SB, TTYPE, 1, IAC, SE]);
        assert_eq!(&reply[..3], [IAC, WILL, TTYPE]);
        assert!(reply.ends_with(b"XTERM-256COLOR\xff\xf0"));
        assert_eq!(t.feed(&[b'a', IAC, IAC, b'\r', 0, b'b']).0, [b'a', IAC, b'\r', b'b']);
    }

    #[test]
    fn typing_escapes_iac_and_sends_enter_as_cr_nul() {
        let mut t = Telnet::new((80, 24));
        assert_eq!(t.encode(&[b'l', b's', b'\r', IAC]), [b'l', b's', b'\r', 0, IAC, IAC]);
        t.feed(&[IAC, DO, BINARY]);
        assert_eq!(t.encode(b"\r"), b"\r");
    }

    #[test]
    fn a_size_of_255_is_escaped() {
        let mut t = Telnet::new((255, 24));
        t.feed(&[IAC, DO, NAWS]);
        assert_eq!(t.window_size(), [IAC, SB, NAWS, 0, IAC, IAC, 0, 24, IAC, SE]);
    }

    #[test]
    fn stored_password_answers_each_prompt_once() {
        let mut a = AutoLogin::new("admin".into(), Some("s3cret".into()));
        assert_eq!(a.seen(b"Welcome\r\n\x1b[1mrouter login:\x1b[0m "), Some("admin\r".into()));
        assert_eq!(a.seen(b"admin\r\nPassword: "), Some("s3cret\r".into()));
        // Refused: the second prompt is left to the operator.
        assert_eq!(a.seen(b"\r\nLogin incorrect\r\nrouter login: "), None);
        assert_eq!(a.seen(b"Password: "), None);
    }

    #[test]
    fn a_device_that_asks_only_for_a_password_gets_it() {
        let mut a = AutoLogin::new("admin".into(), Some("pw".into()));
        assert_eq!(a.seen(b"User Access Verification\r\n\r\nPassword: "), Some("pw\r".into()));
        assert_eq!(a.seen(b"\r\nrouter> Last login: today"), None);
        // Without a stored password only the user is typed.
        let mut none = AutoLogin::new("admin".into(), None);
        assert_eq!(none.seen(b"login: "), Some("admin\r".into()));
        assert_eq!(none.seen(b"Password: "), None);
        assert_eq!(none.seen(b"\r\nlogin: "), None);
    }
}
