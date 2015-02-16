(define factorial (lambda (n)
                    (define loop (lambda (n res)
                                   (if (= n 0)
                                       res
                                       (loop (- n 1) (* res n)))))
                    (loop n 1)))
(factorial 5)
