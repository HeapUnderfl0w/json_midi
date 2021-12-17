mod model;
mod player;
mod trackmode;

use anyhow::Context;
use chrono::Local;
use model::PlayerResult;
use player::MidiPlayer;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Args {
    /// Include meta events
    #[structopt(short, long)]
    meta: bool,

    /// Emit json prettified
    #[structopt(short, long)]
    pretty: bool,

    /// Emit timing information as a delta instead of an absolute timestamp
    #[structopt(short, long)]
    delta: bool,

    /// File to write to, otherwise stdout
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,

    /// The file to convert
    #[structopt(name = "FILE", parse(from_os_str))]
    midi_file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    let midi_file = fs::read(&args.midi_file).context("failed to read midi data into memory")?;
    let smf = midly::Smf::parse(&midi_file).context("failed to parse midi file header")?;

    let stdout = io::stdout();

    let outfile: Box<dyn Write> = match args.output {
        Some(f) => Box::new(fs::File::create(f).context("could not create output file")?),
        None => Box::new(stdout.lock()),
    };

    let player = MidiPlayer::new(&smf, args.meta, args.delta);

    let (p, e, ev) = player
        .into_iter()
        .fold((0, 0, Vec::new()), |(mut p, mut e, mut ev), ne| {
            match ne {
                PlayerResult::Event(v) => {
                    p += 1;
                    e += 1;
                    ev.push(v);
                },
                PlayerResult::Ignored => {
                    p += 1;
                },
            };
            (p, e, ev)
        });

    let track = model::Track {
        generated:        Local::now().to_rfc3339(),
        source_file:      format!("{}", args.midi_file.display()),
        events_processed: p,
        events_emitted:   e,
        emitted_meta:     args.meta,
        events:           ev,
    };

    if args.pretty {
        serde_json::to_writer_pretty(outfile, &track).context("failed to serialize data")?;
    } else {
        serde_json::to_writer(outfile, &track).context("failed to serialize data")?;
    }

    Ok(())
}
