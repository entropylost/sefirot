pub fn bayer(n: usize) -> Vec<u16> {
    assert!(n <= 256);
    if n == 0 {
        panic!("Bayer matrix of order 0 is not defined");
    } else if n == 1 {
        return vec![0];
    }
    let n2 = n / 2;
    let prev_matrix = bayer(n2);
    let mut next_matrix = vec![0; n * n];
    for i in 0..n2 {
        for j in 0..n2 {
            let v = prev_matrix[i * n2 + j] * 4;
            next_matrix[i * n + j] = v;
            next_matrix[i * n + (j + n2)] = v + 2;
            next_matrix[(i + n2) * n + j] = v + 3;
            next_matrix[(i + n2) * n + (j + n2)] = v + 1;
        }
    }
    next_matrix
}
