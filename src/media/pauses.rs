//! When dictation is recognized while the person speaks (`voice.rs`, the gear's "Text while
//! speaking"): at a pause after some speech, the whole speech so far, when it is worth the run. Loudness is
//! measured in 25 ms slices against the recording's own quiet (its tenth percentile), so a noisy
//! room and a quiet one both find their pauses. Pure: the tests below drive it with made-up sound.

/// What the recording since `from` is ready for.
#[derive(Debug, PartialEq)]
pub enum Cut {
    /// Speech then a pause: send `from..at`; the next piece starts at `at`.
    Speech(usize),
    /// Silence only so far: nothing to send; skip to `at`.
    Silence(usize),
}

/// A slice of loudness, in seconds.
const SLICE: f32 = 0.025;
/// How long a pause cuts, how much of it the piece keeps, and the least speech worth a piece.
const PAUSE: f32 = 0.5;
const KEEP: f32 = 0.2;
const MIN_SPEECH: f32 = 0.4;
/// The loudness of a quiet room at most: the floor of a recording with no quiet in it yet.
const QUIET_ROOM: f32 = 0.008;
/// Past this, a piece is cut at its quietest moment of the last `LOOK` seconds, pause or not.
const LONGEST: f32 = 20.;
const LOOK: f32 = 3.;

/// The loudness (RMS) of each 25 ms slice.
fn slices(samples: &[f32], win: usize) -> Vec<f32> {
    samples
        .chunks(win)
        .filter(|c| c.len() == win)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect()
}

/// The 25 ms slices' loudness, how long a slice is, and the levels of silence and of speech in
/// this recording.
fn measure(samples: &[f32], rate: u32) -> (Vec<f32>, usize, f32, f32) {
    let win = ((rate as f32 * SLICE) as usize).max(1);
    let all = slices(samples, win);
    // The recording's quiet: its tenth percentile (no more than a quiet room's, for a recording
    // that has been speech all along); silence is a little above it, speech above that.
    let mut sorted = all.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted.get(sorted.len() / 10).copied().unwrap_or(0.).min(QUIET_ROOM);
    let silent = (floor * 3.).max(0.003);
    (all, win, silent, silent * 2.)
}

/// Whether there was speech since sample `from` (stopping sends the speech again only then).
pub fn spoke_since(samples: &[f32], rate: u32, from: usize) -> bool {
    let (all, win, _, loud) = measure(samples, rate);
    let speech = all.iter().skip(from / win).filter(|l| **l > loud).count();
    speech as f32 * SLICE >= MIN_SPEECH
}

/// Where to cut what was recorded since `from` (`samples` is the whole recording); None: not yet.
pub fn cut(samples: &[f32], rate: u32, from: usize) -> Option<Cut> {
    let (all, win, silent, loud) = measure(samples, rate);
    if all.is_empty() || from >= samples.len() {
        return None;
    }
    let first = from / win;
    let now = &all[first.min(all.len())..];
    let n = |secs: f32| (secs / SLICE) as usize;
    let at = |slice: usize| (first + slice) * win;
    let pause = n(PAUSE);
    // The first pause after enough speech; checks that come late still cut at the first.
    let (mut spoken, mut quiet) = (0, 0);
    for (i, l) in now.iter().enumerate() {
        if *l > loud {
            spoken += 1;
        }
        quiet = if *l < silent { quiet + 1 } else { 0 };
        if quiet >= pause && spoken >= n(MIN_SPEECH) {
            return Some(Cut::Speech(at(i + 1 - pause + n(KEEP))));
        }
    }
    let spoke = spoken >= n(MIN_SPEECH);
    // No speech, and silence lasting: skipped, nothing to send.
    if !spoke && quiet >= pause {
        return Some(Cut::Silence(at(now.len() - pause + n(KEEP))));
    }
    // Too long without a pause: at the quietest slice of the last seconds.
    if now.len() >= n(LONGEST) {
        let look = n(LOOK).min(now.len());
        let start = now.len() - look;
        let quiet = (start..now.len()).min_by(|a, b| now[*a].total_cmp(&now[*b])).unwrap_or(now.len());
        return Some(if spoke { Cut::Speech(at(quiet)) } else { Cut::Silence(at(quiet)) });
    }
    None
}

/// The least new speech worth a run, in seconds.
const MIN_NEW: f32 = 0.8;

/// Seconds of work per second of speech before anything is measured.
const FIRST_GUESS: f32 = 0.3;

/// How long a run of `total` seconds of speech will take, from the runs measured so far
/// ((seconds of speech, seconds it took), the latest last): a straight line through them by least
/// squares — on one machine the time grows about linearly with the speech, with a fixed part (the
/// request, the audio's decoding) — once they span enough length to draw one; from one run, its
/// rate; from none, a guess. Never less than the latest rate says for the fixed part's sake.
pub fn predict(runs: &[(f32, f32)], total: f32) -> f32 {
    let rate = |(s, t): (f32, f32)| t / s.max(0.1);
    let Some(last) = runs.last().copied() else {
        return FIRST_GUESS * total;
    };
    let n = runs.len() as f32;
    let (mx, my) = (runs.iter().map(|r| r.0).sum::<f32>() / n, runs.iter().map(|r| r.1).sum::<f32>() / n);
    let sxx: f32 = runs.iter().map(|r| (r.0 - mx).powi(2)).sum();
    // Lengths too alike (or one run) draw no line: the rate.
    if runs.len() < 2 || sxx < 4. {
        return rate(last) * total;
    }
    let slope = runs.iter().map(|r| (r.0 - mx) * (r.1 - my)).sum::<f32>() / sxx;
    let line = my + slope.max(0.) * (total - mx);
    line.max(0.)
}

/// At a pause, whether to recognize the whole speech again: worth it when the speech added since
/// the last run is at least what the run will take (`predict`). Short dictation updates at nearly
/// every pause; long dictation, whose every run costs more, less often — the processor never falls
/// behind the person, and a long dictation is not sent again at every breath.
pub fn worth_a_run(new_secs: f32, predicted: f32) -> bool {
    new_secs >= MIN_NEW && new_secs >= predicted
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    /// `secs` of a 200 Hz tone at `amp` (speech) or of faint noise (`amp` 0).
    fn sound(secs: f32, amp: f32) -> Vec<f32> {
        (0..(secs * RATE as f32) as usize)
            .map(|i| {
                let noise = ((i * 7919 % 101) as f32 / 101. - 0.5) * 0.002;
                noise + amp * (i as f32 * 200. * std::f32::consts::TAU / RATE as f32).sin()
            })
            .collect()
    }

    fn join(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.concat()
    }

    #[test]
    fn speech_then_a_pause_is_cut_inside_the_pause() {
        let s = join(&[sound(1.5, 0.3), sound(0.7, 0.)]);
        match cut(&s, RATE, 0) {
            Some(Cut::Speech(at)) => {
                let secs = at as f32 / RATE as f32;
                assert!((1.5..1.9).contains(&secs), "cut at {secs}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn speech_still_going_or_a_short_breath_waits() {
        assert_eq!(cut(&sound(2., 0.3), RATE, 0), None);
        let s = join(&[sound(1.5, 0.3), sound(0.3, 0.), sound(0.5, 0.3)]);
        assert_eq!(cut(&s, RATE, 0), None, "a breath is not a pause");
    }

    #[test]
    fn silence_alone_is_skipped_not_sent() {
        let s = join(&[sound(1.5, 0.3), sound(0.7, 0.), sound(2., 0.)]);
        let Some(Cut::Speech(first)) = cut(&s, RATE, 0) else { panic!() };
        assert!(matches!(cut(&s, RATE, first), Some(Cut::Silence(_))));
    }

    #[test]
    fn the_next_piece_is_cut_at_its_own_pause() {
        let s = join(&[sound(1.5, 0.3), sound(0.7, 0.), sound(2., 0.25), sound(0.7, 0.)]);
        let Some(Cut::Speech(first)) = cut(&s, RATE, 0) else { panic!() };
        match cut(&s, RATE, first) {
            Some(Cut::Speech(at)) => assert!(at > first + RATE as usize * 2),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_run_is_worth_it_when_the_new_speech_outweighs_its_cost() {
        assert!(worth_a_run(3., 1.9), "short dictation: at nearly every pause");
        assert!(!worth_a_run(0.5, 0.4), "too little new");
        assert!(!worth_a_run(5., 14.), "a minute in: 14 s of work for 5 s new");
        assert!(worth_a_run(15., 14.));
    }

    #[test]
    fn a_run_is_predicted_from_the_runs_measured() {
        // Nothing yet: a guess; one run: its rate.
        assert!((predict(&[], 10.) - 3.).abs() < 0.01);
        assert!((predict(&[(5., 1.)], 20.) - 4.).abs() < 0.01);
        // Several, as on dev (0.6B): about 0.2 s per second plus a fixed part — the line, not
        // the last rate, which a short run's fixed part inflates.
        let runs = [(5.5, 1.0), (11.5, 2.3), (16.9, 3.9), (22.1, 5.0)];
        let p = predict(&runs, 60.);
        assert!((13.0..15.5).contains(&p), "60 s predicted at {p}");
        // Lengths too alike draw no line: the latest rate.
        assert!((predict(&[(10., 2.), (10.5, 2.4)], 20.) - 20. * 2.4 / 10.5).abs() < 0.01);
    }

    #[test]
    fn stopping_after_silence_is_not_new_speech() {
        let s = join(&[sound(3., 0.3), sound(0.7, 0.), sound(4., 0.)]);
        let after_run = (3.7 * RATE as f32) as usize;
        assert!(!spoke_since(&s, RATE, after_run), "only silence after the last run");
        let s = join(&[sound(3., 0.3), sound(0.7, 0.), sound(1.5, 0.3)]);
        assert!(spoke_since(&s, RATE, after_run), "words after it");
    }

    #[test]
    fn a_long_stretch_without_a_pause_is_cut_anyway() {
        assert!(matches!(cut(&sound(21., 0.3), RATE, 0), Some(Cut::Speech(_))));
    }
}
