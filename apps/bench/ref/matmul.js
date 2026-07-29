function matmul(n){ const a=new Int32Array(n*n),b=new Int32Array(n*n),c=new Int32Array(n*n);
  for(let i=0;i<n;i++) for(let j=0;j<n;j++){ a[i*n+j]=(i+j)%100; b[i*n+j]=(i*j)%100; }
  for(let i=0;i<n;i++) for(let j=0;j<n;j++){ let s=0; for(let k=0;k<n;k++) s+=a[i*n+k]*b[k*n+j]; c[i*n+j]=s; }
  let sum=0; for(let i=0;i<n*n;i++) sum=(sum+c[i])%1000000007;
  return sum; }
console.log(matmul(400));
