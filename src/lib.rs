use std::fmt::{self, Display};
use std::io::{stdin, Read};

use mailparse::MailAddrList;

/// A representation of a mail. In this case, it's left unparsed. If you make few changes, it's
/// slow to parse and then serialize it, so this provides a speedy alternative.
pub struct UnparsedMail {
    contents: Vec<u8>,

    from: MailAddrList,
    to: MailAddrList,

    recipients: Option<mailparse::MailAddrList>,
    sender: Option<mailparse::MailAddrList>,
    cc: Option<mailparse::MailAddrList>,
    bcc: Option<mailparse::MailAddrList>,
    subject: Option<String>,
    user_agent: Option<String>,
}
macro_rules! get_header_addr {
    ($name:ident, $field:ident, $header:literal) => {
        fn $name(&mut self) -> &mailparse::MailAddrList {
            if self.$field.is_some() {
                log::info!(
                    "Got cached {}: {}",
                    stringify!($field),
                    self.$field.as_ref().unwrap()
                );
                return self.$field.as_ref().unwrap();
            }

            let v = (|| {
                let header = self.get_header_raw(concat!("\n", $header))?;
                mailparse::addrparse_header(&header).ok()
            })()
            .unwrap_or_else(|| mailparse::MailAddrList::from(Vec::new()));
            self.$field.insert(v)
        }
    };
}
impl UnparsedMail {
    pub fn new(buf: impl Into<Vec<u8>>, from: MailAddrList, to: MailAddrList) -> Self {
        Self {
            contents: buf.into(),

            from,
            to,

            recipients: None,
            bcc: None,
            cc: None,
            sender: None,

            subject: None,
            user_agent: None,
        }
    }
    /// Read from stdin and CLI arguments. Useful when using postfix.
    ///
    /// Returns `None` is `stdin` isn't connected.
    pub fn from_stdin() -> Option<Self> {
        let mut stdin = stdin();
        let mut buf = Vec::with_capacity(128);
        stdin.read_to_end(&mut buf).ok()?;

        let mut args = std::env::args().skip(1);
        if args.next().as_deref() != Some("-f") {
            eprintln!("First argument has to be -f followed by the sender");
            std::process::exit(1);
        }
        let from = args.next().unwrap();
        if args.next().as_deref() != Some("--") {
            eprintln!("Third argument has to be -- followed by recipients");
            std::process::exit(1);
        }
        let mut to = args.fold(String::new(), |mut acc, v| {
            acc += &v;
            acc += ", ";
            acc
        });
        // pop last ", "
        to.pop();
        to.pop();

        log::info!("From {from}, to {to}");

        let from = mailparse::addrparse(&from).expect("Failed to parse from emails");
        let to = mailparse::addrparse(&to).expect("Failed to parse from emails");

        Some(Self::new(buf, from, to))
    }

    /// Header has to start with `\n`
    fn get_header_idx(&self, header: &str) -> Option<usize> {
        // also search for end of headers to return early from search
        log::info!("Searching for header {header:?}");
        let needle = aho_corasick::AhoCorasickBuilder::new()
            .ascii_case_insensitive(true)
            .build([header, "\r\n\r\n", "\n\n"])
            .unwrap();
        let first = needle.find(&self.contents)?;

        if first.pattern().as_i32() != 0 {
            return None;
        }
        Some(first.start() + 2)
    }
    /// Header has to start with `\n`
    fn get_header_raw(&self, header: &str) -> Option<mailparse::MailHeader> {
        let b = &self.contents[self.get_header_idx(header)?..];
        let (header, _) = mailparse::parse_header(b).ok()?;
        Some(header)
    }

    get_header_addr!(get_recipients, recipients, "to:");
    get_header_addr!(get_sender, sender, "from:");
    get_header_addr!(get_cc, cc, "cc:");
    get_header_addr!(get_bcc, bcc, "bcc:");

    fn get_subject(&mut self) -> &str {
        if self.subject.is_some() {
            return self.subject.as_ref().unwrap();
        }
        let v = (|| {
            let header = self.get_header_raw("\nsubject:")?;
            Some(header.get_value())
        })()
        .unwrap_or_default();
        self.subject.insert(v)
    }
    fn get_ua(&mut self) -> &str {
        if self.user_agent.is_some() {
            return self.user_agent.as_ref().unwrap();
        }
        let v = (|| {
            let header = self.get_header_raw("\nuser-agent:")?;
            Some(header.get_value())
        })()
        .unwrap_or_default();
        self.user_agent.insert(v)
    }
}
impl BasicMail for UnparsedMail {
    fn into_parts(self) -> (Vec<u8>, MailAddrList, MailAddrList) {
        (self.contents, self.from, self.to)
    }

    fn header_domain(&mut self) -> Option<&str> {
        let addr = utils::iter_addrs(self.header_recipients()).next()?;
        let idx = addr.addr.find('@')?;
        let domain = &addr.addr[idx + 1..];
        log::info!("Got domain: {domain}");
        Some(domain)
    }
    fn domain(&mut self) -> Option<&str> {
        let addr = utils::iter_addrs(self.recipients()).next()?;
        let idx = addr.addr.find('@')?;
        let domain = &addr.addr[idx + 1..];
        log::info!("Got domain: {domain}");
        Some(domain)
    }
    fn header_recipients(&mut self) -> &MailAddrList {
        let addrs = self.get_recipients();
        log::info!("Got header recipients: {addrs}");
        addrs
    }
    fn header_sender(&mut self) -> &MailAddrList {
        let addrs = self.get_sender();
        log::info!("Got header senders: {addrs}");
        addrs
    }
    fn recipients(&mut self) -> &MailAddrList {
        let addrs = &self.to;
        log::info!("Got recipients: {addrs}");
        addrs
    }
    fn sender(&mut self) -> &MailAddrList {
        let addrs = &self.from;
        log::info!("Got senders: {addrs}");
        addrs
    }
    fn cc(&mut self) -> &MailAddrList {
        self.get_cc()
    }
    fn bcc(&mut self) -> &MailAddrList {
        self.get_bcc()
    }
    fn subject(&mut self) -> &str {
        self.get_subject()
    }
    fn user_agent(&mut self) -> Option<&str> {
        let s = self.get_ua();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
    fn set_header(&mut self, header: &str, s: &str) {
        (|| {
            let header = format!("\n{header}");
            let idx = self.get_header_idx(&header)?;
            let (header, _end) = mailparse::parse_header(&self.contents[idx..]).ok()?;
            let current_len = header.get_value_raw().len();
            let start_value =
                idx + memchr::memmem::find(&self.contents[idx..], header.get_value_raw()).unwrap();
            let end_value = start_value + current_len;
            // shorter
            if current_len > s.len() {
                self.contents
                    .copy_within(end_value.., end_value + s.len() - current_len);
                self.contents
                    .truncate(self.contents.len() + s.len() - current_len);
            } else {
                log::info!(
                    "Move {end_value}..{} -> {} (len = {})",
                    self.contents.len() + current_len - s.len(),
                    end_value + s.len() - current_len,
                    self.contents.len(),
                );
                let previous_len = self.contents.len();
                // extend
                self.contents
                    .resize(self.contents.len() + s.len() - current_len, 0);
                self.contents
                    .copy_within(end_value..previous_len, end_value + s.len() - current_len);
            }
            self.contents[start_value..(start_value + s.len())].copy_from_slice(s.as_bytes());
            Some(())
        })();
    }

    fn set_recipient(
        &mut self,
        recipients: impl Into<MailAddrList>,
        disclosure: RecipientDisclosure,
    ) {
        let recipients = recipients.into();
        match disclosure {
            RecipientDisclosure::Open => {
                self.set_header("to", &recipients.to_string());
            }
            RecipientDisclosure::Undisclosed { name } => {
                self.set_header("to", &format!("{name} <>"));
            }
            RecipientDisclosure::Keep => {}
            RecipientDisclosure::Sender { name } => {
                let mut sender = utils::iter_addrs(self.header_sender()).next();
                if sender.is_none() {
                    sender = utils::iter_addrs(self.sender()).next();
                }
                let sender = sender
                    .map_or("noreply@localhost", |sender| &sender.addr)
                    .to_owned();
                self.set_header("to", &format!("{name} <{sender}>",));
            }
        }
        self.to = recipients;
    }
}

/// Action after filter.
/// Also accepts:
/// - bool: true => Continue, false => Ignore
/// - Option<()>: Some(()) => Continue, None => Ignore (useful when you have an option in the
///   filter and want to use `?` on it)
/// - Result<(), [`Error`]>: Ok(()) => Continue, Err(err) => Reject(err)
pub enum Action {
    Continue,
    Ignore,
    Reject(Error),
}
impl From<bool> for Action {
    fn from(value: bool) -> Self {
        if value {
            Self::Continue
        } else {
            Self::Ignore
        }
    }
}
impl From<Option<()>> for Action {
    fn from(o: Option<()>) -> Self {
        if o.is_some() {
            Self::Continue
        } else {
            Self::Ignore
        }
    }
}
impl From<Result<(), Error>> for Action {
    fn from(r: Result<(), Error>) -> Self {
        match r {
            Ok(()) => Self::Continue,
            Err(e) => Self::Reject(e),
        }
    }
}
type FilterFn<M> = Box<dyn Fn(&mut M) -> Action>;

/// Mail filter
pub struct Filter<M: BasicMail> {
    filters: Vec<FilterFn<M>>,
}
impl<M: BasicMail> Filter<M> {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Filter the mail
    ///
    /// The return type means you can use this in all the same places as [`Self::and_then`] &
    /// [`Self::map`], but the code's intentions can become more clear when using those functions.
    pub fn filter<V: Into<Action>>(&mut self, filter: impl Fn(&mut M) -> V + 'static) -> &mut Self {
        self.filters.push(Box::new(move |mail| filter(mail).into()));
        self
    }
    /// Either continue or reject mail
    pub fn and_then(&mut self, f: impl Fn(&mut M) -> Result<(), Error> + 'static) -> &mut Self {
        self.filter(f)
    }
    /// Change mail contents
    pub fn map(&mut self, f: impl Fn(&mut M) + 'static) -> &mut Self {
        self.filter(move |mail| {
            f(mail);
            true
        })
    }

    /// Filter a mail and return the result.
    /// If `Err`, reject the mail.
    pub fn process(&self, mut mail: M) -> Result<(Vec<u8>, MailAddrList, MailAddrList), String> {
        let mut e = None;
        for (idx, filter) in self.filters.iter().enumerate() {
            log::info!("Running transformation n:r {}", idx + 1);
            match filter(&mut mail) {
                Action::Continue => {
                    log::info!("Continue!");
                }
                Action::Ignore => {
                    log::info!("Filtered out at n:r {}", idx + 1);
                    return Ok(mail.into_parts());
                }
                Action::Reject(err) => {
                    log::info!("Reject at n:r {}: {}", idx + 1, err);
                    e = Some(err);
                    break;
                }
            }
        }
        log::info!("Every transformation complete. Error? {}", e.is_some());

        if let Some(err) = e {
            Err(err.to_string())
        } else {
            let (body, from, to) = mail.into_parts();
            log::info!("From {from}, to {to}");
            Ok((body, from, to))
        }
    }
}
impl<M: BasicMail> Default for Filter<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// SMTP error message
pub struct Error {
    /// Status: <https://en.wikipedia.org/wiki/List_of_SMTP_server_return_codes>
    pub status: u16,
    /// The message after the status. Can be anything you like, really
    pub message: String,
}
impl Error {
    /// Standard unauthorized message: `530: 5.7.0 Authentication required`
    pub fn unauthorized() -> Self {
        Self {
            status: 530,
            message: String::from("5.7.0 Authentication required"),
        }
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status, self.message)
    }
}

/// How to show the other recipients to the [new recipients](BasicMail::set_recipient).
pub enum RecipientDisclosure {
    /// Disclose all recipients when overriding them.
    /// Changes the header to match with the new recipients.
    /// Bad if you want to keep other recipients private.
    Open,
    /// Email will be displayed as just to `name`.
    Undisclosed { name: String },
    /// Keep the `to` address of the original mail.
    Keep,
    /// Set the address of the receiver as the sender, with the name `name`.
    Sender { name: String },
}
impl RecipientDisclosure {
    /// The "standard" undisclosed recipient option.
    pub fn undisclosed_recipients() -> Self {
        Self::Undisclosed {
            name: String::from("Undisclosed Recipients"),
        }
    }
}
/// The domain/recipient/sender can be different in the headers & info from mail server.
pub trait BasicMail {
    /// Into body + from + to
    fn into_parts(self) -> (Vec<u8>, MailAddrList, MailAddrList);

    /// Get the domain of the first recipient, according to the headers
    fn header_domain(&mut self) -> Option<&str>;
    /// Get the domain of the first recipient, according to the mail server's recipients
    fn domain(&mut self) -> Option<&str>;
    fn header_recipients(&mut self) -> &mailparse::MailAddrList;
    fn header_sender(&mut self) -> &mailparse::MailAddrList;
    fn recipients(&mut self) -> &mailparse::MailAddrList;
    fn sender(&mut self) -> &mailparse::MailAddrList;
    fn cc(&mut self) -> &mailparse::MailAddrList;
    fn bcc(&mut self) -> &mailparse::MailAddrList;
    fn subject(&mut self) -> &str;
    fn user_agent(&mut self) -> Option<&str>;

    /// Please note that the senders and recipients cannot be changed using the headers. Consider
    /// [`BasicMail::set_recipient`] or methods on implementers.
    fn set_header(&mut self, header: &str, s: &str);
    /// Set recipient header & to sendmail.
    ///
    /// See [`BasicMail::set_header`].
    fn set_recipient(
        &mut self,
        recipients: impl Into<MailAddrList>,
        disclosure: RecipientDisclosure,
    );
}
/// Functions only allowed on parsed mails.
///
/// Some operations are difficult to do on unparsed mails, so this exports some more advanced
/// features.
pub trait StructuredMail: BasicMail {}

/// Helper functions for working with types from [`mailparse`].
pub mod utils {
    use mailparse::{MailAddr, MailAddrList, SingleInfo};

    /// Iterate over all the addresses of a [`MailAddrList`], returned from many functions of
    /// [`crate::BasicMail`].
    pub fn iter_addrs(addrs: &MailAddrList) -> impl Iterator<Item = &SingleInfo> {
        addrs.iter().flat_map(|addr| match addr {
            MailAddr::Single(s) => std::slice::from_ref(s).iter(),
            MailAddr::Group(group) => group.addrs.iter(),
        })
    }
    /// Create a [`MailAddrList`] from an iterator of addresses.
    pub fn addr_list_from_iter(iter: impl Iterator<Item = SingleInfo>) -> MailAddrList {
        MailAddrList::from(iter.map(MailAddr::Single).collect::<Vec<_>>())
    }
    /// Create a [`MailAddrList`] from a single address.
    pub fn addr_single(addr: impl Into<String>) -> MailAddrList {
        addr_list_from_iter(
            [SingleInfo {
                addr: addr.into(),
                display_name: None,
            }]
            .into_iter(),
        )
    }
}
