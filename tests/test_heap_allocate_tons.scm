(define allocate-tons (lambda (n xs)
                        (if (eq? n 0)
                            xs
                            (allocate-tons (- n 1) (cons n xs)))))

;; This should always be larger than `heap::DEFAULT_CONS_CAPACITY`.
(define n 10000)

;; Cause a bunch of arena allocations.
(define xs (allocate-tons n '()))

;; And then it should all get collected, and then allocated again.
(set! xs '())
(set! xs (allocate-tons n '()))
