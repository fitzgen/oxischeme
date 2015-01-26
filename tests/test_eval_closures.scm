;; To test closures, we use the church encoding of cons cells.
(define cons (lambda (x y)
               (lambda (m) (m x y))))
(define car (lambda (z)
              (z (lambda (p q) p))))
(define cdr (lambda (z)
              (z (lambda (p q) q))))

(define x (cons 2 (cons 1 '())))
(car (cdr x))
