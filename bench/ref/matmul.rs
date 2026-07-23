fn matmul(n:usize)->i64{ let mut a=vec![0i64;n*n]; let mut b=vec![0i64;n*n]; let mut c=vec![0i64;n*n];
  for i in 0..n { for j in 0..n { a[i*n+j]=((i+j)%100) as i64; b[i*n+j]=((i*j)%100) as i64; } }
  for i in 0..n { for j in 0..n { let mut s=0i64; for k in 0..n { s+=a[i*n+k]*b[k*n+j]; } c[i*n+j]=s; } }
  let mut sum=0i64; for i in 0..n*n { sum=(sum+c[i])%1000000007; } sum }
fn main(){ println!("{}", matmul(400)); }
