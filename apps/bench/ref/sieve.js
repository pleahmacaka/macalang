function countPrimes(n){ const m=new Uint8Array(n+1); let c=0;
  for(let i=2;i<=n;i++) if(!m[i]){c++; for(let j=i+i;j<=n;j+=i) m[j]=1;} return c; }
console.log(countPrimes(10000000));
