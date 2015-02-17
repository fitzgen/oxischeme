(define cycle (cons 1 '()))
(set-cdr! cycle cycle)
(print cycle)
