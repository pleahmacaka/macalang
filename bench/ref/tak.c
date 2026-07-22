#include <stdio.h>
long tak(long x,long y,long z){ return y<x ? tak(tak(x-1,y,z),tak(y-1,z,x),tak(z-1,x,y)) : z; }
int main(){ printf("%ld\n", tak(32,16,8)); return 0; }
