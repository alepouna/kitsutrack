use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use iht_protocol::{FLAG_TRACKING, PosePacket};
use std::{
    f32::consts::PI,
    io::Write,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(about = "Synthetic iPhone tracking source using the production protocol")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:4243")]
    listen: String,
    #[arg(long, default_value_t = 60.0)]
    rate: f32,
    #[arg(long, default_value_t = 25.0)]
    amplitude_degrees: f32,
    #[arg(long, default_value_t = 0.2)]
    amplitude_metres: f32,
    #[arg(long, default_value_t = 0.25)]
    frequency: f32,
    #[arg(long, default_value_t = 0.0)]
    packet_loss: f32,
    /// Pose stream to emit. These scenarios are useful for downstream diagnostics.
    #[arg(long, value_enum, default_value_t = Scenario::Sweep)]
    scenario: Scenario,
    /// Stop after this many seconds; by default the simulator runs until interrupted.
    #[arg(long)]
    duration: Option<f32>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Scenario {
    /// Smooth head movement with translation (the default).
    Sweep,
    /// A stable identity pose, useful for checking connection and baseline noise.
    Stationary,
    /// Periodically withhold tracking flags to exercise tracking-loss recovery.
    Loss,
    /// Add a short, large yaw excursion to exercise jump rejection.
    Jump,
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.rate > 0.0
            && (0.0..=1.0).contains(&args.packet_loss)
            && args.duration.is_none_or(|duration| duration > 0.0),
        "invalid rate, packet loss, or duration"
    );
    let listener =
        TcpListener::bind(&args.listen).with_context(|| format!("listen on {}", args.listen))?;
    eprintln!("synthetic tracker listening on {}", args.listen);
    loop {
        let (mut stream, peer) = listener.accept()?;
        stream.set_nodelay(true)?;
        eprintln!("bridge connected from {peer}");
        let start = Instant::now();
        let interval = Duration::from_secs_f32(1.0 / args.rate);
        for sequence in 0_u64.. {
            let t = start.elapsed().as_secs_f32();
            if args.duration.is_some_and(|duration| t >= duration) {
                break;
            }
            let phase = 2.0 * PI * args.frequency * t;
            let is_tracking = !matches!(args.scenario, Scenario::Loss) || (t % 4.0 < 2.5);
            let yaw_degrees = match args.scenario {
                Scenario::Sweep => args.amplitude_degrees * phase.sin(),
                Scenario::Stationary | Scenario::Loss => 0.0,
                Scenario::Jump => {
                    if t % 5.0 < 0.15 {
                        args.amplitude_degrees * 8.0
                    } else {
                        0.0
                    }
                }
            };
            let yaw = yaw_degrees.to_radians();
            let half = yaw / 2.0;
            let packet = PosePacket {
                flags: if is_tracking { FLAG_TRACKING } else { 0 },
                sequence,
                timestamp_ns: start.elapsed().as_nanos() as u64,
                rotation: [0.0, half.sin(), 0.0, half.cos()],
                translation: if matches!(args.scenario, Scenario::Stationary | Scenario::Loss) {
                    [0.0; 3]
                } else {
                    [
                        args.amplitude_metres * phase.sin(),
                        args.amplitude_metres * (phase * 0.7).sin(),
                        args.amplitude_metres * (phase * 0.5).cos(),
                    ]
                },
            };
            // Deterministic loss makes diagnostics and tests reproducible.
            let drop_every = if args.packet_loss == 0.0 {
                0
            } else {
                (1.0 / args.packet_loss).round() as u64
            };
            if (drop_every == 0 || sequence % drop_every != 0)
                && stream.write_all(&packet.encode()).is_err()
            {
                break;
            }
            thread::sleep(interval.saturating_sub(start.elapsed() - Duration::from_secs_f32(t)));
        }
        if args.duration.is_some() {
            break;
        }
    }
    Ok(())
}
