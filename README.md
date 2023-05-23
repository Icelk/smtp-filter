# smtp-filter

`smtp-filter` is a framework for making fast
[Postfix SMTP filters](https://www.postfix.org/FILTER_README.html) (after-queue) in Rust.

Mail filters can modify all the elements of mails, enabling the following (not exhaustive list):

-   Custom, programmable mailing lists
-   Adding or changing content in the body
-   Virus checks
-   Modifying the header fields (changing receiver, subject (e.g. remove "Re: "), hiding user-agent)
-   Triggering back-end events from any incoming mail

## Example

```rust
use std::io::Write;

use mailparse::SingleInfo;
use smtp_filter::{utils, BasicMail, Filter, RecipientDisclosure, UnparsedMail};

fn main() {
    // log to file when built with --release
    #[cfg(not(debug_assertions))]
    let log_file = std::fs::File::create("/var/spool/filter/mail.log").unwrap();

    let mut logger = env_logger::builder();
    logger
        .filter_level(log::LevelFilter::Info)
        .parse_default_env();
    #[cfg(not(debug_assertions))]
    logger.target(env_logger::Target::Pipe(Box::new(log_file)));
    logger.init();

    let mut filter = Filter::new();
    filter
        // if mail is to someone at `icelk.dev`
        .filter(|mail: &mut UnparsedMail| mail.header_domain() == Some("mydomain.org"))
        // if there are exactly 1 recipient
        .filter(|mail| mail.header_recipients().count_addrs() == 1)
        .and_then(|mail| {
            let sender = utils::iter_addrs(mail.sender()).next();
            // only allow mail from "special@myotherdomain.org"
            let authorized = sender.map_or(false, |s| s.addr == "special@myotherdomain.org");
            // extract the user of the recipient
            let recip = utils::iter_addrs(mail.header_recipients())
                .next()
                .unwrap()
                .addr
                .strip_suffix("@mydomain.org")
                .to_owned();
            match recip {
                // reject mail if not from special mail address
                _ if !authorized => Err(smtp_filter::Error::unauthorized()),
                Some("some-user") => {
                    // re-route mail to other emails
                    mail.set_recipient(
                        utils::addr_list_from_iter(
                            [
                                SingleInfo {
                                    addr: "info@myotherdomain.org".into(),
                                    display_name: None,
                                },
                                SingleInfo {
                                    addr: "accounting@mydomain.org".into(),
                                    display_name: Some("Accounting".into()),
                                },
                            ]
                            .into_iter(),
                        ),
                        // Show all other recipients to all recipients, so you can continue the thread
                        RecipientDisclosure::Open,
                    );
                    Ok(())
                }
                _ => Ok(()),
            }
        });
    // read mail
    let mail = UnparsedMail::from_stdin().unwrap();
    match filter.process(mail) {
        Ok((mail, from, to)) => {
            // send the mail back to postfix
            let mut child = std::process::Command::new("sendmail")
                .args(["-f", &from.to_string(), "--", &to.to_string()])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .expect("failed to run sendmail");
            let mut stdin = child.stdin.take().unwrap();
            stdin
                .write_all(&mail)
                .expect("failed to send mail to sendmail");
            stdin.flush().unwrap();
            drop(stdin);
            let status = child.wait().expect("sendmail failed");
            std::process::exit(status.code().unwrap_or(0));
        }
        Err(s) => {
            // if err, reject mail
            println!("{s}");
            std::process::exit(1);
        }
    }
}

```

## TODO

-   `set_header` with new headers in `UnparsedMail`
-   `ParsedMail`
-   Bin feature which handles logging & spawn sendmail
