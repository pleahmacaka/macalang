package main
import "fmt"
func mandel(size int) int64 { var total int64=0
  for py:=0;py<size;py++ { for px:=0;px<size;px++ {
    x0:=float64(px)/float64(size)*3.5-2.5; y0:=float64(py)/float64(size)*2.0-1.0
    x,y:=0.0,0.0; it:=int64(0)
    for it<1000 && x*x+y*y<=4.0 { xt:=x*x-y*y+x0; y=2*x*y+y0; x=xt; it++ }
    total+=it } }; return total }
func main(){ fmt.Println(mandel(800)) }
