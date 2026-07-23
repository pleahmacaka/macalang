fn mandel(size:i64)->i64{ let mut total=0i64;
  for py in 0..size { for px in 0..size {
    let x0=px as f64/size as f64*3.5-2.5; let y0=py as f64/size as f64*2.0-1.0;
    let (mut x,mut y)=(0.0f64,0.0f64); let mut it=0i64;
    while it<1000 && x*x+y*y<=4.0 { let xt=x*x-y*y+x0; y=2.0*x*y+y0; x=xt; it+=1; }
    total+=it; } } total }
fn main(){ println!("{}", mandel(800)); }
