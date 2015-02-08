(define allocate-tons (lambda (n xs)
                        (if (eq? n 0)
                            xs
                            (allocate-tons (- n 1) (cons n xs)))))

;; This should always be larger than `heap::DEFAULT_CONS_CAPACITY`.
(allocate-tons 3000 '())
