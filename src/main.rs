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

    /// Dump the parsed object instead of scanning events
    #[structopt(long)]
    dump: bool,

    #[structopt(long, name = "DEBUGF")]
    debug: Option<String>
}

struct DbgWriter {
    d: Option<std::fs::File>
}

impl DbgWriter {
    pub fn n(v: Option<String>) -> Self {
        Self {
            d: v.map(|x| std::fs::File::create(x).expect("fatal: failed to create debug file"))
        }
    }

    pub fn w(&mut self, t: &'static str, s: String) {
        if let Some(f) = self.d.as_mut() {
            let _ = writeln!(f, "############### {}", t);
            let _ = writeln!(f, "{}", s);
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    let mut dbg = DbgWriter::n(args.debug.clone());
    dbg.w("args", format!("{:#?}", args));

    let midi_file = fs::read(&args.midi_file).context("failed to read midi data into memory")?;
    dbg.w("file", format!("read length {}", midi_file.len()));

    let smf = midly::Smf::parse(&midi_file).context("failed to parse midi file header")?;

    dbg.w("midi.header", format!("{:#?}", smf.header));

    let stdout = io::stdout();

    let sd = match args.output {
        Some(mut f) => {
            let f1 = f.clone();

            let fp = f
                .file_name()
                .context("the filename cannot be ..")?
                .to_string_lossy()
                .to_string();
            f.pop();
            let fpath = f.join(format!("{}.tmp", fp));

            Some((fpath, f1))
        },
        None => None,
    };

    let outfile: Box<dyn Write> = match sd.as_ref() {
        Some((f, _)) => Box::new(fs::File::create(f).context("could not create output file")?),
        None => Box::new(stdout.lock()),
    };

    if args.dump {
        // rebind outfile as mutable
        let mut outfile = outfile;
        write!(outfile, "{:#?}", smf).context("write failed")?;
        return Ok(());
    }

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

    if let Some((s, d)) = sd {
        fs::rename(s, d).context("failed to move tmp file over target")?;
    }

    Ok(())
}
