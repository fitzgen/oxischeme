(define foo (lambda () (set! x 5)))
(define x 1)
(foo)
x
