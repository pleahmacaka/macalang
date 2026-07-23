fn ack(m:i64,n:i64)->i64{ if m==0 {n+1} else if n==0 {ack(m-1,1)} else {ack(m-1,ack(m,n-1))} }
fn main(){ println!("{}", ack(3,10)); }
