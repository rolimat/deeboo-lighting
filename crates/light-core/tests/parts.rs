use light_core::parts::parts;

fn r(a: u8, b: u8) -> Vec<u8> {
    (a..=b).collect()
}

fn union(sets: &[&[u8]]) -> Vec<u8> {
    let mut v: Vec<u8> = sets.iter().flat_map(|s| s.iter().copied()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn parts_examples() {
    assert_eq!(parts(8).p, [r(1, 2), r(3, 4), r(5, 6), r(7, 8)]);
    assert_eq!(parts(12).p, [r(1, 3), r(4, 6), r(7, 9), r(10, 12)]);
    let p6 = parts(6);
    assert_eq!(p6.p, [r(1, 2), r(3, 4), r(5, 6), r(3, 4)]);
    assert_eq!(p6.ends, [1, 2, 5, 6]);
    assert_eq!(p6.centre, [3, 4]);
    let p10 = parts(10);
    assert_eq!(p10.p, [r(1, 2), r(3, 5), r(6, 8), r(9, 10)]);
    assert_eq!(p10.ends, [1, 2, 9, 10]);
    assert_eq!(p10.centre, r(3, 8));
    let p7 = parts(7);
    assert_eq!(p7.p, [vec![1], r(2, 4), r(4, 6), vec![7]]);
    assert_eq!(p7.h1, r(1, 4));
    assert_eq!(p7.h2, r(4, 7));
    assert_eq!(parts(9).p, [r(1, 3), r(4, 6), r(7, 9), r(4, 6)]);
    assert_eq!(parts(1).p, [vec![], vec![1], vec![1], vec![]]);
    assert_eq!(parts(2).p, [vec![], vec![1], vec![2], vec![]]);
    assert_eq!(parts(3).p, [vec![1], vec![2], vec![3], vec![2]]);
}

#[test]
fn parts_partition_n_1_to_16() {
    for n in 1..=16u8 {
        let q = parts(n);
        let all = r(1, n);
        assert_eq!(q.all, all, "n={n}");
        assert_eq!(union(&[&q.p[0], &q.p[1], &q.p[2], &q.p[3]]), all, "n={n}");
        assert_eq!(union(&[&q.ends, &q.centre]), all, "n={n}");
        assert!(q.ends.iter().all(|s| !q.centre.contains(s)), "n={n}");
        assert_eq!(union(&[&q.h1, &q.h2]), all, "n={n}");
        let inter: Vec<u8> = q.h1.iter().copied().filter(|s| q.h2.contains(s)).collect();
        let expect = if n % 2 == 1 {
            vec![n.div_ceil(2)]
        } else {
            vec![]
        };
        assert_eq!(inter, expect, "n={n}");
        assert_eq!(union(&[&q.odd, &q.even]), all, "n={n}");
        if n % 3 != 0 || n % 4 == 0 {
            assert_eq!(q.p[0].len(), q.p[3].len(), "n={n}");
            assert_eq!(q.p[1].len(), q.p[2].len(), "n={n}");
            let mirrored: Vec<u8> = q.p[0].iter().rev().map(|s| n + 1 - s).collect();
            assert_eq!(q.p[3], mirrored, "n={n}");
        }
    }
}

#[test]
fn three_section_rule_only_without_multiple_of_4() {
    for n in [3, 6, 9, 15] {
        let q = parts(n);
        assert_eq!(q.p[3], q.p[1], "n={n}");
    }
    let q = parts(12);
    assert_ne!(q.p[3], q.p[1]);
}
