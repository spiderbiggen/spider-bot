use std::mem::swap;
use unicode_segmentation::UnicodeSegmentation;

pub fn edit_distance<S: AsRef<str>, S2: AsRef<str>>(a: S, b: S2) -> usize {
    let a_ref = a.as_ref();
    let b_ref = b.as_ref();
    if a_ref == b_ref {
        return 0;
    }
    let a_chars: Vec<&str> = a_ref.graphemes(true).collect();
    let b_chars: Vec<&str> = b_ref.graphemes(true).collect();

    match (a_chars.len(), b_chars.len()) {
        (size, 0) => size,
        (0, size) => size,
        (a_len, b_len) => {
            let mut v0 = vec![0; b_len + 1];
            let mut v1 = vec![0; b_len + 1];
            for i in 0..b_len {
                v0[i] = i;
            }

            for i in 0..a_len {
                v1[0] = i + 1;

                for j in 0..b_len {
                    let d_cost = v0[j + 1] + 1;
                    let i_cost = v1[j] + 1;
                    let s_cost = v0[j]
                        + if a_chars.get(i) == b_chars.get(i) {
                            0
                        } else {
                            1
                        };

                    v1[j + 1] = d_cost.min(i_cost).min(s_cost);
                }
                swap(&mut v0, &mut v1);
            }
            v0[b_len]
        }
    }
}
