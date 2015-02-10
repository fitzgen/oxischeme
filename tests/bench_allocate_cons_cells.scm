(define alloc
  (lambda (n xs)
    (if (eq? n 0)
        xs
        (alloc (- n 1) (cons n xs)))))

alloc
