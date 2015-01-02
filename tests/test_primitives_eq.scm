(define a (cons 1 2))

(cons (eq? a a)
      (eq? a (cons 1 2)))
