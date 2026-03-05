use crate::plan::types::PlaybackSegment;

#[allow(dead_code)]
pub fn output_to_source_time(output_ms: f64, segments: &[PlaybackSegment]) -> f64 {
    for segment in segments {
        if output_ms >= segment.output_start_ms && output_ms <= segment.output_end_ms {
            let output_offset = output_ms - segment.output_start_ms;
            let source_offset = output_offset * segment.playback_rate;
            return segment.source_start_ms + source_offset;
        }
    }

    if let Some(last) = segments.last() {
        return last.source_end_ms;
    }

    output_ms
}

#[allow(dead_code)]
pub fn source_to_output_time(source_ms: f64, segments: &[PlaybackSegment]) -> f64 {
    for segment in segments {
        if source_ms >= segment.source_start_ms && source_ms <= segment.source_end_ms {
            let source_offset = source_ms - segment.source_start_ms;
            let output_offset = source_offset / segment.playback_rate;
            return segment.output_start_ms + output_offset;
        }
    }

    if let Some(last) = segments.last() {
        return last.output_end_ms;
    }

    source_ms
}

#[allow(dead_code)]
pub fn segment_at_output_time<'a>(
    output_ms: f64,
    segments: &'a [PlaybackSegment],
    inclusive_end: bool,
) -> Option<&'a PlaybackSegment> {
    for segment in segments {
        let in_range = if inclusive_end {
            output_ms >= segment.output_start_ms && output_ms <= segment.output_end_ms
        } else {
            output_ms >= segment.output_start_ms && output_ms < segment.output_end_ms
        };
        if in_range {
            return Some(segment);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::types::SegmentType;

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1e-6,
            "expected {expected}, got {actual} (delta {delta})"
        );
    }

    fn sample_segments() -> Vec<PlaybackSegment> {
        vec![
            PlaybackSegment {
                segment_type: SegmentType::Gap,
                source_start_ms: 0.0,
                source_end_ms: 1000.0,
                source_duration_ms: 1000.0,
                output_start_ms: 0.0,
                output_end_ms: 250.0,
                output_duration_ms: 250.0,
                playback_rate: 4.0,
            },
            PlaybackSegment {
                segment_type: SegmentType::Action,
                source_start_ms: 1000.0,
                source_end_ms: 2000.0,
                source_duration_ms: 1000.0,
                output_start_ms: 250.0,
                output_end_ms: 1250.0,
                output_duration_ms: 1000.0,
                playback_rate: 1.0,
            },
            PlaybackSegment {
                segment_type: SegmentType::Gap,
                source_start_ms: 2000.0,
                source_end_ms: 5000.0,
                source_duration_ms: 3000.0,
                output_start_ms: 1250.0,
                output_end_ms: 1850.0,
                output_duration_ms: 600.0,
                playback_rate: 5.0,
            },
        ]
    }

    #[test]
    fn maps_output_to_source() {
        let segments = sample_segments();
        assert_close(output_to_source_time(0.0, &segments), 0.0);
        assert_close(output_to_source_time(125.0, &segments), 500.0);
        assert_close(output_to_source_time(500.0, &segments), 1250.0);
        assert_close(output_to_source_time(1700.0, &segments), 4250.0);
        assert_close(output_to_source_time(1850.0, &segments), 5000.0);
    }

    #[test]
    fn maps_source_to_output() {
        let segments = sample_segments();
        assert_close(source_to_output_time(0.0, &segments), 0.0);
        assert_close(source_to_output_time(500.0, &segments), 125.0);
        assert_close(source_to_output_time(1500.0, &segments), 750.0);
        assert_close(source_to_output_time(3000.0, &segments), 1450.0);
        assert_close(source_to_output_time(5000.0, &segments), 1850.0);
    }

    #[test]
    fn finds_segment_at_output_time() {
        let segments = sample_segments();
        let seg = segment_at_output_time(600.0, &segments, true).unwrap();
        assert_eq!(seg.segment_type, SegmentType::Action);

        let seg2 = segment_at_output_time(1300.0, &segments, true).unwrap();
        assert_eq!(seg2.segment_type, SegmentType::Gap);
    }
}
