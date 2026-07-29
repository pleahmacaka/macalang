#include <stdio.h>
#include <stdlib.h>
int main(void){ long n=10000000, count=0; char*m=calloc(n+1,1);
  for(long i=2;i<=n;i++) if(!m[i]){count++; for(long j=i+i;j<=n;j+=i) m[j]=1;}
  printf("%ld\n",count); return 0; }
