use anyhow::{bail, Context, Result};
use scuttlebutt::field::FiniteField as FF;
mod row;
use row::{add_rows, Row};

/*

⎡100110...| y_1 ⎤
⎢110100...| y_2 ⎥
⎣010101...| y_3 ⎦

↓

⎡100001...| v_1 ⎤
⎢010010...| v_2 ⎥
⎣000111...| v_3 ⎦

Then, below equations are satisfied. (v_i, R_i \in F)

R_1 + R_6 + ...= v_1
R_2 + R_5 + ...= v_2
R_4 + R_5 + R_6 + ... = v_3

So, after generating R_i (i \in 1..m, i != 1, 2, 4) <-$ F, we can decide R_1, R_2, R_4 by

R_1 = v_1 - R_6 - ...
R_2 = v_2 - R_5 - ...
R_4 = v_1 - R_5 - R_6 - ...

R_3 also can be random value because you have to decide R_i properly if and only if there are at least 1 row of the original matrix its i-th value is 1.

*/

pub fn gaussian_elimination<F: FF>(
    matrix: Vec<(Vec<bool>, F)>,
) -> Result<Option<Vec<(usize, Vec<bool>, F)>>> {
    check_matrix(&matrix).with_context(|| format!("@{}:{}", file!(), line!()))?;

    let n = matrix.len();
    let m = matrix[0].0.len();

    let mut matrix = matrix
        .into_iter()
        .map(|(row, target)| Row::new(row, target))
        .collect::<Vec<_>>();

    let mut first_indices = Vec::with_capacity(n);

    let mut i = 0;
    let mut j = 0;

    while i < n {
        if j >= m {
            // bail!("matrix is not full rank");
            return Ok(None);
        }

        let mut t = None;
        for k in i..n {
            if matrix[k][j] {
                t = Some(k);
                break;
            }
        }

        let Some(t) = t else {
            j += 1;

            if j >= m {
                // bail!("matrix is not full rank");
                return Ok(None);
            }

            continue;
        };

        matrix.swap(i, t);

        for k in 0..n {
            if k != i && matrix[k][j] {
                add_rows(&mut matrix, k, i, j..m);
            }
        }

        first_indices.push(j);

        i += 1;
        j += 1;
    }

    let res = first_indices
        .into_iter()
        .zip(matrix.into_iter())
        .map(|(i, row)| (i, row.values, row.target))
        .collect::<Vec<_>>();

    Ok(Some(res))
}

fn check_matrix<F: FF>(matrix: &[(Vec<bool>, F)]) -> Result<()> {
    let n = matrix.len();

    if n == 0 {
        bail!("matrix is empty");
    }

    let m = matrix[0].0.len();

    if n > m {
        bail!("matrix row is more than column");
    }

    if m == 0 {
        bail!("matrix row is empty");
    }

    for (i, (row, _)) in matrix.iter().enumerate() {
        if row.len() != m {
            bail!("matrix row {} has different length", i);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::set_utils::FromU128;

    use super::*;
    use num_traits::Zero;
    use rand::Rng;
    use scuttlebutt::{field::F128b, AesRng};

    fn inner<RNG: Rng>(n: usize, m: usize, rng: &mut RNG, verbose: bool) -> bool {
        let mut matrix = Vec::with_capacity(n);
        for _ in 0..n {
            let mut row = Vec::with_capacity(m);
            for _ in 0..m {
                row.push(rng.gen());
            }
            let target: F128b = rng.gen();
            matrix.push((row, target));
        }

        let res = match gaussian_elimination(matrix.clone()) {
            Ok(Some(res)) => res,
            Ok(None) => {
                if verbose {
                    println!("[No Solution] matrix: {:?}", matrix);
                }
                return false;
            }
            Err(e) => {
                panic!("error: {}", e);
            }
        };

        if verbose {
            println!("matrix: {:?}", matrix);
            println!("res: {:?}", res);
        }

        let mut vec_r: Vec<F128b> = (0..m).map(|_| rng.gen()).collect::<Vec<_>>();

        for (i, row, target) in res.iter() {
            let mut sum = F128b::zero();
            let mut checker = F128b::zero();

            for (j, val) in row.iter().enumerate() {
                if *i != j && *val {
                    sum += vec_r[j];
                    checker += vec_r[j];
                }
            }
            sum += *target;
            vec_r[*i] = sum;

            checker += vec_r[*i];

            assert_eq!(checker, *target);
        }

        for (row, target) in matrix.iter() {
            let mut sum = F128b::zero();
            for (j, val) in row.iter().enumerate() {
                if *val {
                    sum += vec_r[j];
                }
            }
            assert_eq!(sum, *target);
        }

        return true;
    }

    fn random_test_successful<RNG: Rng>(
        min: usize,
        max: usize,
        rng: &mut RNG,
        verbose: bool,
    ) -> bool {
        let n = rng.gen_range(min..=max);
        let m = rng.gen_range((2 * n)..=(max + 2 * n));

        inner(n, m, rng, verbose)
    }

    fn random_test_possible_failure<RNG: Rng>(
        min: usize,
        max: usize,
        rng: &mut RNG,
        verbose: bool,
    ) -> bool {
        let n = rng.gen_range(min..=max);
        let m = rng.gen_range(n..=max);

        inner(n, m, rng, verbose)
    }

    #[test]
    fn random_small_test() {
        let mut rng = AesRng::new();
        random_test_successful(3, 10, &mut rng, true);
    }

    #[test]
    fn random_large_test() {
        let mut rng = AesRng::new();

        let mut success_count = 0;
        let mut fail_count = 0;

        for _ in 0..500 {
            let res = random_test_successful(10, 100, &mut rng, false);

            if res {
                success_count += 1;
            } else {
                fail_count += 1;
            }
        }

        println!(
            "[successful] success: {}, fail: {}",
            success_count, fail_count
        );

        let mut success_count = 0;
        let mut fail_count = 0;

        for _ in 0..500 {
            let res = random_test_possible_failure(10, 100, &mut rng, false);

            if res {
                success_count += 1;
            } else {
                fail_count += 1;
            }
        }

        println!(
            "[possible failure] success: {}, fail: {}",
            success_count, fail_count
        );
    }

    #[test]
    fn test_edge_case_1() {
        let matrix = vec![(vec![true, false, false], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix).unwrap().unwrap();

        assert_eq!(
            res,
            vec![(0, vec![true, false, false], F128b::from_u128(1))]
        );
    }

    #[test]
    fn test_edge_case_2() {
        let matrix = vec![(vec![false, false, true], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix).unwrap().unwrap();

        assert_eq!(
            res,
            vec![(2, vec![false, false, true], F128b::from_u128(1))]
        );
    }

    #[test]
    fn test_edge_case_3() {
        let matrix = vec![(vec![true, false, false], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix).unwrap().unwrap();

        assert_eq!(
            res,
            vec![(0, vec![true, false, false], F128b::from_u128(1))]
        );
    }

    #[test]
    fn test_edge_case_4() {
        let matrix = vec![(vec![true], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix).unwrap().unwrap();

        assert_eq!(res, vec![(0, vec![true], F128b::from_u128(1))]);
    }

    #[test]
    fn test_err_1() {
        let matrix = vec![(vec![], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix);

        assert!(res.is_err());
    }

    #[test]
    fn test_no_solution_0() {
        let matrix = vec![(vec![false, false, false], F128b::from_u128(1))];

        let res = gaussian_elimination(matrix).unwrap();

        assert!(res.is_none());
    }

    #[test]
    fn test_no_solution_1() {
        let matrix = vec![
            (vec![true, false, false], F128b::from_u128(1)),
            (vec![false, false, false], F128b::from_u128(1)),
        ];

        let res = gaussian_elimination(matrix).unwrap();

        assert!(res.is_none());
    }

    #[test]
    fn test_no_solution_2() {
        let matrix = vec![
            (vec![true, false, false], F128b::from_u128(1)),
            (vec![true, false, false], F128b::from_u128(1)),
        ];

        let res = gaussian_elimination(matrix).unwrap();

        assert!(res.is_none());
    }
}
