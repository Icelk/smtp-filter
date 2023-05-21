# smtp-filter

`smtp-filter` is a framework for making fast
[Postfix SMTP filters](https://www.postfix.org/FILTER_README.html) (after-queue) in Rust.

## TODO

-   advertise capabilities (mailing lists, changing body, checking for viruses, changing headers, triggering back-end events based on mails)
-   Example
-   Example filters
-   `set_header` with new headers in `UnparsedMail`
-   Docs
-   `ParsedMail`
-   Bin feature which handles logging & spawn sendmail
