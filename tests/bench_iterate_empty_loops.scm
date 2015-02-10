(define iter (lambda (n)
               (if (eq? n 0)
                   '()
                   (iter (- n 1)))))
iter
