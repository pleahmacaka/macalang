#include <stdio.h>
int main(void){ int size=800; long total=0;
  for(int py=0;py<size;py++) for(int px=0;px<size;px++){
    double x0=(double)px/size*3.5-2.5, y0=(double)py/size*2.0-1.0, x=0,y=0; int it=0;
    while(it<1000 && x*x+y*y<=4.0){ double xt=x*x-y*y+x0; y=2*x*y+y0; x=xt; it++; }
    total+=it; }
  printf("%ld\n",total); return 0; }
