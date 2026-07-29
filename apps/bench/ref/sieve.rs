fn count_primes(n:usize)->i64{ let mut m=vec![0u8;n+1]; let mut c=0i64;
  let mut i=2; while i<=n { if m[i]==0 { c+=1; let mut j=i+i; while j<=n { m[j]=1; j+=i; } } i+=1; } c }
fn main(){ println!("{}", count_primes(10000000)); }
