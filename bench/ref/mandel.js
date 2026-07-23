function mandel(size){ let total=0;
  for(let py=0;py<size;py++) for(let px=0;px<size;px++){
    const x0=px/size*3.5-2.5, y0=py/size*2.0-1.0; let x=0,y=0,it=0;
    while(it<1000 && x*x+y*y<=4.0){ const xt=x*x-y*y+x0; y=2*x*y+y0; x=xt; it++; }
    total+=it; } return total; }
console.log(mandel(800));
