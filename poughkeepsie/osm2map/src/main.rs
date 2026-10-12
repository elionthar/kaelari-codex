use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use osm2map::{convert, overpass_query, write_ppm, Overpass, DEFAULT_BBOX, DEFAULT_M_PER_TILE};
use pkmap::BBox;

const USAGE: &str = "\
osm2map: download downtown Poughkeepsie from the Overpass API and build a .pkm tile map.

USAGE: cargo run --release -p osm2map -- [OPTIONS]

OPTIONS:
  --bbox S,W,N,E       area to fetch (default: downtown Poughkeepsie)
  --m-per-tile N       meters per tile (default: 2)
  --raw PATH           where the raw Overpass JSON is cached (default: data/overpass.json)
  --out PATH           output map (default: assets/poughkeepsie.pkm)
  --preview PATH       also write a 1-pixel-per-tile PPM image (default: data/preview.ppm)
  --endpoint URL       Overpass endpoint (default: https://overpass-api.de/api/interpreter)
  --refetch            download again even if the raw file exists
  --print-query        print the Overpass QL and exit
";

struct Args {
    bbox: BBox,
    m_per_tile: f32,
    raw: PathBuf,
    out: PathBuf,
    preview: PathBuf,
    endpoint: String,
    refetch: bool,
    print_query: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        bbox: DEFAULT_BBOX,
        m_per_tile: DEFAULT_M_PER_TILE,
        raw: "data/overpass.json".into(),
        out: "assets/poughkeepsie.pkm".into(),
        preview: "data/preview.ppm".into(),
        endpoint: "https://overpass-api.de/api/interpreter".into(),
        refetch: false,
        print_query: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--bbox" => {
                let v: Vec<f64> = val()?.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>().map_err(|e| format!("--bbox: {e}"))?;
                let [south, west, north, east] = v[..] else { return Err("--bbox needs 4 numbers".into()) };
                if south >= north || west >= east {
                    return Err("--bbox must be S,W,N,E with S<N and W<E".into());
                }
                a.bbox = BBox { south, west, north, east };
            }
            "--m-per-tile" => a.m_per_tile = val()?.parse().map_err(|e| format!("--m-per-tile: {e}"))?,
            "--raw" => a.raw = val()?.into(),
            "--out" => a.out = val()?.into(),
            "--preview" => a.preview = val()?.into(),
            "--endpoint" => a.endpoint = val()?,
            "--refetch" => a.refetch = true,
            "--print-query" => a.print_query = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    Ok(a)
}

fn fetch(args: &Args) -> Result<(), String> {
    let query = overpass_query(&args.bbox);
    eprintln!("Downloading from {} (this can take a minute)...", args.endpoint);
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .timeout(Duration::from_secs(300))
        .user_agent("poughkeepsie-zombies-osm2map/0.1")
        .build();
    let resp = agent
        .post(&args.endpoint)
        .send_form(&[("data", &query)])
        .map_err(|e| format!("Overpass request failed: {e}"))?;
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut body).map_err(|e| format!("reading response: {e}"))?;
    if let Some(dir) = args.raw.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.raw, &body).map_err(|e| format!("writing {}: {e}", args.raw.display()))?;
    eprintln!("Saved {} KiB to {}", body.len() / 1024, args.raw.display());
    Ok(())
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.print_query {
        print!("{}", overpass_query(&args.bbox));
        return Ok(());
    }
    if args.refetch || !args.raw.exists() {
        fetch(&args)?;
    } else {
        eprintln!("Using cached {} (pass --refetch to download again)", args.raw.display());
    }
    let text = std::fs::read(&args.raw).map_err(|e| format!("reading {}: {e}", args.raw.display()))?;
    let data: Overpass = serde_json::from_slice(&text).map_err(|e| {
        let head = String::from_utf8_lossy(&text[..text.len().min(300)]).into_owned();
        format!("{} is not valid Overpass JSON ({e}). Start of file:\n{head}", args.raw.display())
    })?;
    if text.windows(6).take(4096).any(|w| w == b"remark") {
        eprintln!("warning: Overpass returned a remark (possibly a timeout); the data may be incomplete");
    }

    let (map, report) = convert(&data, args.bbox, args.m_per_tile);
    if let Some(dir) = args.out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?);
    map.write_to(&mut file).map_err(|e| e.to_string())?;
    drop(file);
    if let Some(dir) = args.preview.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    write_ppm(&map, &args.preview).map_err(|e| e.to_string())?;

    println!("{report}");
    println!("Wrote {} ({} KiB) and preview {}", args.out.display(), std::fs::metadata(&args.out).map(|m| m.len() / 1024).unwrap_or(0), args.preview.display());
    if report.stats.buildings == 0 || report.stats.roads == 0 {
        return Err("no buildings or roads found: check the bbox and the raw response".into());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
