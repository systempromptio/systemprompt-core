//! The whole of a headless invocation: arguments in, exit status out.
//!
//! A binary hands over its argument list without the program name, its three
//! standard streams and a cancellation token it fires on interrupt. `run`
//! writes its event stream to the output; `login` reads the token from the
//! input and writes one JSON line. A usage error is written to the error
//! stream with the usage text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{BufRead, Write};

use systemprompt_traits::CancellationToken;

use crate::command::args::{Command, parse};
use crate::command::event::EventWriter;
use crate::command::exit::ExitStatus;
use crate::command::host::HeadlessHost;
use crate::command::login::{login, read_token};
use crate::command::run::run;
use crate::command::usage::{USAGE, UsageError};

/// The three standard streams of an invocation.
#[derive(Debug)]
pub struct Streams<R, W, E> {
    pub input: R,
    pub output: W,
    pub errors: E,
}

fn usage(errors: &mut impl Write, error: &UsageError) -> ExitStatus {
    if let Err(failure) = writeln!(errors, "{error}\n\n{USAGE}") {
        tracing::warn!(error = %failure, "the usage error could not be written");
    }
    ExitStatus::Usage
}

pub async fn execute<R, W, E>(
    host: &HeadlessHost,
    args: Vec<String>,
    streams: Streams<R, W, E>,
    cancel: &CancellationToken,
) -> ExitStatus
where
    R: BufRead + Send,
    W: Write + Send,
    E: Write + Send,
{
    let Streams {
        mut input,
        mut output,
        mut errors,
    } = streams;
    match parse(args) {
        Err(error) => usage(&mut errors, &error),
        Ok(Command::Run(args)) => match args.load() {
            Err(error) => usage(&mut errors, &error),
            Ok(request) => {
                let mut out = EventWriter::new(output);
                Box::pin(run(host, request, cancel, &mut out)).await.status
            },
        },
        Ok(Command::Login(args)) => match read_token(&mut input) {
            Err(error) => usage(&mut errors, &error),
            Ok(token) => {
                let report = login(host, args.request(token)).await;
                match report.encode() {
                    Ok(line) => {
                        if let Err(failure) = writeln!(output, "{line}") {
                            tracing::warn!(error = %failure, "the login line could not be written");
                        }
                    },
                    Err(failure) => {
                        tracing::warn!(error = %failure, "the login line could not be encoded");
                    },
                }
                report.status
            },
        },
    }
}
