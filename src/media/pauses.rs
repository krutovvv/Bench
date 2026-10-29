//! Where dictation is cut while the person speaks (`voice.rs`, the gear's "Text while speaking"):
//! at a pause after some speech, so each piece is recognized while the next is said. Loudness is
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

/// Where to cut what was recorded since `from` (`samples` is the whole recording); None: not yet.
pub fn cut(samples: &[f32], rate: u32, from: usize) -> Option<Cut> {
    let win = ((rate as f32 * SLICE) as usize).max(1);
    let all = slices(samples, win);
    if all.is_empty() || from >= samples.len() {
        return None;
    }
    // The recording's quiet: its tenth percentile (no more than a quiet room's, for a recording
    // that has been speech all along); silence is a little above it, speech above that.
    let mut sorted = all.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 10].min(QUIET_ROOM);
    let silent = (floor * 3.).max(0.003);
    let loud = silent * 2.;
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
    fn a_long_stretch_without_a_pause_is_cut_anyway() {
        assert!(matches!(cut(&sound(21., 0.3), RATE, 0), Some(Cut::Speech(_))));
    }
}
