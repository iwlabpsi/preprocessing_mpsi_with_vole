use anyhow::{bail, Result};
use scuttlebutt::field::FiniteField as FF;

// not use.
// What we really want is a simultaneous equation seeking solution for bit sparse matrices with more columns than rows, which is difficult to achieve with LU decomposition.
// 和訳: 本当に欲しいのは行より列が多いビット疎行列の連立方程式求解であり、これはLU分解で達成するのは難しいと考えられる。

// http://www.ced.is.utsunomiya-u.ac.jp/lecture/2011/prog/p2/kadai3/no3/lu.pdf

fn lu_decomp<F: FF>(i: usize, a: &mut [Vec<F>], l: &mut [Vec<F>], u: &mut [Vec<F>]) -> Result<()> {
    if i >= a.len() {
        return Ok(());
    }

    if a[i][i] == F::zero() {
        bail!("a[{}][{}] is zero", i, i);
    }

    l[i][i] = a[i][i];

    for j in (i + 1)..a.len() {
        l[j][i] = a[j][i];
        u[i][j] = a[i][j] / l[i][i];
    }

    for j in (i + 1)..a.len() {
        for k in (i + 1)..a.len() {
            a[j][k] -= l[j][i] * u[i][k];
        }
    }

    lu_decomp(i + 1, a, l, u)?;

    Ok(())
}

// Ly = b
fn solve_y<F: FF>(l: &[Vec<F>], b: &[F]) -> Result<Vec<F>> {
    let mut y = vec![F::zero(); b.len()];

    for i in 0..b.len() {
        if l[i][i] == F::zero() {
            bail!("l[{}][{}] is zero", i, i);
        }

        y[i] = b[i];
        for j in 0..i {
            let t = y[j];
            y[i] -= l[i][j] * t;
        }
        y[i] /= l[i][i];
    }

    Ok(y)
}

// Ux = y
fn solve_x<F: FF>(u: &[Vec<F>], y: &[F]) -> Vec<F> {
    let mut x = vec![F::zero(); y.len()];

    for i in (0..y.len()).rev() {
        x[i] = y[i];
        for j in (i + 1)..y.len() {
            let t = x[j];
            x[i] -= u[i][j] * t;
        }
    }

    x
}

pub fn solve<F: FF>(mut a: Vec<Vec<F>>, b: &[F]) -> Result<Vec<F>> {
    let size = a.len();

    if size != a[0].len() {
        bail!("Matrix is not square");
    }

    let mut l = vec![vec![F::zero(); size]; size];
    let mut u = vec![vec![F::zero(); size]; size];

    lu_decomp(0, &mut a, &mut l, &mut u)?;

    let y = solve_y(&l, b)?;
    let x = solve_x(&u, &y);

    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::solve;
    use num_traits::Zero;
    use rand::Rng;
    use scuttlebutt::field::F128b;
    use scuttlebutt::AesRng;

    #[test]
    fn test_solve() {
        let mut rng = AesRng::new();

        let size = 100;

        let mut a = vec![vec![F128b::zero(); size]; size];
        let mut b = vec![F128b::zero(); size];

        for i in 0..size {
            for j in 0..size {
                a[i][j] = rng.gen();
            }
            b[i] = rng.gen();
        }

        let x = solve(a.clone(), &b).unwrap();

        for i in 0..size {
            let mut sum = F128b::zero();
            for j in 0..size {
                sum += a[i][j] * x[j];
            }

            println!("[{}]: {:?}", i, sum);
            assert_eq!(sum, b[i]);
        }
    }
}
