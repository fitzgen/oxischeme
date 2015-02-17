(define list-of-values
  (lambda (exps env)
    (if (no-operands? exps)
        '()
        (cons (eval (first-operand exps) env)
              (list-of-values (rest-operands exps) env)))))

(define my-apply
  (lambda (procedure arguments)
    (if (primitive-procedure? procedure)
        (apply-primitive-procedure procedure arguments)
        (if (compound-procedure? procedure)
            (eval-sequence (procedure-body procedure)
                           (extend-environment (procedure-parameters procedure)
                                               arguments
                                               (procedure-environment procedure)))
            (error "my-apply: Unknown procedure type:" procedure)))))

(define true? (lambda (x) (eq? x #t)))

(define eval-if
  (lambda (exp env)
    (if (true? (eval (if-predicate exp) env))
        (eval (if-consequent exp) env)
        (eval (if-alternative exp) env))))

(define eval-sequence
  (lambda (exps env)
    (if (last-exp? exps)
        (eval (first-exp exps) env)
        (begin (eval (first-exp exps) env)
               (eval-sequence (rest-exps exps) env)))))

(define eval-assignment
  (lambda (exp env)
    (set-variable-value! (assignment-variable exp)
                         (eval (assignment-value exp) env)
                         env)
    'ok))

(define eval-definition
  (lambda (exp env)
    (define-variable!
      (definition-variable exp)
      (eval (definition-value exp) env)
      env)
    'ok))

(define self-evaluating?
  (lambda (exp)
    (if (number? exp)
        #t
        (if (string? exp)
            #t
            #f))))

(define variable? (lambda (exp) (symbol? exp)))

(define tagged-list?
  (lambda (list sym)
    (if (pair? list)
        (eq? (car list) sym)
        #f)))

(define quoted?
  (lambda (exp)
    (tagged-list? exp 'quote)))

(define text-of-quotation
  (lambda (exp)
    (car (cdr exp))))

(define assignment?
  (lambda (exp)
    (tagged-list? exp 'set!)))

(define assignment-variable
  (lambda (exp)
    (car (cdr exp))))

(define assignment-value
  (lambda (exp)
    (car (cdr (cdr exp)))))

(define definition?
  (lambda (exp)
    (tagged-list? exp 'define)))

(define definition-variable
  (lambda (exp)
    (if (symbol? (car (cdr exp)))
        (car (cdr exp))
        (car (car (cdr exp))))))

(define definition-value
  (lambda (exp)
    (if (symbol? (car (cdr exp)))
        (car (cdr (cdr exp)))
        (make-lambda (cdr (car (cdr exp))) ; formal parameters
                     (cdr (cdr exp))))))   ; body

(define lambda?
  (lambda (exp)
    (tagged-list? exp 'lambda)))

(define lambda-parameters
  (lambda (exp)
    (car (cdr exp))))

(define lambda-body
  (lambda (exp)
    (cdr (cdr exp))))

(define make-lambda
  (lambda (parameters body)
    (cons 'lambda (cons parameters body))))

(define if?
  (lambda (exp)
    (tagged-list? exp 'if)))

(define if-predicate
  (lambda (exp)
    (car (cdr exp))))

(define if-consequent
  (lambda (exp)
    (car (cdr (cdr exp)))))

(define if-alternative
  (lambda (exp)
    (if (not (null? (cdr (cdr (cdr exp)))))
        (car (cdr (cdr (cdr exp))))
        #f)))

(define begin?
  (lambda (exp)
    (tagged-list? exp 'begin)))

(define begin-actions
  (lambda (exp)
    (cdr exp)))

(define last-exp?
  (lambda (seq)
    (null? (cdr seq))))

(define first-exp
  (lambda (seq)
    (car seq)))

(define rest-exps
  (lambda (seq)
    (cdr seq)))

(define application?
  (lambda (exp)
    (pair? exp)))

(define operator
  (lambda (exp)
    (car exp)))

(define operands
  (lambda (exp)
    (cdr exp)))

(define no-operands?
  (lambda (ops)
    (null? ops)))

(define first-operand
  (lambda (ops)
    (car ops)))

(define rest-operands
  (lambda (ops)
    (cdr ops)))

(define true?
  (lambda (x)
    (not (eq? x #f))))

(define false?
  (lambda (x)
    (eq? x #f)))

(define make-procedure
  (lambda (parameters body env)
    (list 'procedure parameters body env)))

(define compound-procedure?
  (lambda (p)
    (tagged-list? p 'procedure)))

(define procedure-parameters
  (lambda (p)
    (car (cdr p))))

(define procedure-body
  (lambda (p)
    (car (cdr (cdr p)))))

(define procedure-environment
  (lambda (p)
    (car (cdr (cdr (cdr p))))))

(define enclosing-environment
  (lambda (env)
    (cdr env)))

(define first-frame
  (lambda (env)
    (car env)))

(define the-empty-environment '())

(define make-frame
  (lambda (variables values)
    (cons variables values)))

(define frame-variables
  (lambda (frame)
    (car frame)))

(define frame-values
  (lambda (frame)
    (cdr frame)))

(define add-binding-to-frame!
  (lambda (var val frame)
    (set-car! frame (cons var (car frame)))
    (set-cdr! frame (cons val (cdr frame)))))

(define extend-environment
  (lambda (vars vals base-env)
    (if (= (length vars) (length vals))
      (cons (make-frame vars vals) base-env)
      (if (< (length vars) (length vals))
          (error "Too many arguments supplied" vars vals)
          (error "Too few arguments supplied" vars vals)))))

(define lookup-variable-value
  (lambda (var env)
    (define env-loop (lambda (env)
                       (define scan (lambda (vars vals)
                                      (if (null? vars)
                                          (env-loop (enclosing-environment env))
                                          (if (eq? var (car vars))
                                              (car vals)
                                              (scan (cdr vars) (cdr vals))))))
                       (if (eq? env the-empty-environment)
                           (error "Unbound variable" var)
                           (begin (define frame (first-frame env))
                                  (scan (frame-variables frame)
                                        (frame-values frame))))))
    (env-loop env)))

(define set-variable-value!
  (lambda (var val env)
    (define env-loop (lambda (env)
                       (define scan (lambda (vars vals)
                                      (if (null? vars)
                                          (env-loop (enclosing-environment env))
                                          (if (eq? var (car vars))
                                              (set-car! vals val)
                                              (scan (cdr vars) (cdr vals))))))
                       (if (eq? env the-empty-environment)
                           (error "Unbound variable - SET!" var)
                           (begin (define frame (first-frame env))
                                  (scan (frame-variables frame)
                                        (frame-values frame))))))
    (env-loop env)))

(define define-variable!
  (lambda (var val env)
    (define frame (first-frame env))
    (define scan
      (lambda (vars vals)
        (if (null? vars)
            (add-binding-to-frame! var val frame)
            (if (eq? var (car vars))
                (set-car! vals val)
                (scan (cdr vars) (cdr vals))))))
    (scan (frame-variables frame)
          (frame-values frame))))

(define primitive-procedure?
  (lambda (proc)
    (tagged-list? proc 'primitive)))


(define primitive-implementation
  (lambda (proc)
    (car (cdr proc))))

(define primitive-procedures
  (list (list 'car car)
        (list 'cdr cdr)
        (list 'cons cons)
        (list 'null? null?)
        (list '+ +)
        (list '- -)
        (list '* *)
        (list '/ /)
        (list '= =)
        (list 'eq? eq?)
        (list '< <)
        (list '> >)
        ;; more primitives...
        ))

(define map
  (lambda (f xs)
    (if (null? xs)
        '()
        (cons (f (car xs))
              (map f (cdr xs))))))

(define primitive-procedure-names
  (lambda ()
    (map car primitive-procedures)))


(define primitive-procedure-objects
  (lambda ()
    (map (lambda (proc) (list 'primitive (car (cdr proc))))
         primitive-procedures)))

(define apply-primitive-procedure
  (lambda (proc args)
    (apply (primitive-implementation proc) args)))

(define input-prompt ";;; M-Eval input:")
(define output-prompt ";;; M-Eval value:")

(define prompt-for-input
  (lambda (string)
    (print)
    (print)
    (print string)))

(define announce-output
  (lambda (string)
    (print)
    (print string)))

(define user-print
  (lambda (object)
    (if (compound-procedure? object)
        (print (list 'compound-procedure
                       (procedure-parameters object)
                       (procedure-body object)
                       '<procedure-env>))
        (print object))))

(define driver-loop
  (lambda ()
    (prompt-for-input input-prompt)
    (define input (read))
    (define output (eval input the-global-environment))
    (announce-output output-prompt)
    (user-print output)
    (driver-loop)))

(define setup-environment
  (lambda ()
    (define initial-env (extend-environment (primitive-procedure-names)
                                            (primitive-procedure-objects)
                                            the-empty-environment))
    (define-variable! 'true #t initial-env)
    (define-variable! 'false #f initial-env)
    initial-env))

(define the-global-environment (setup-environment))

(define eval
  (lambda (exp env)
    (if (self-evaluating? exp) exp
        (if (variable? exp) (lookup-variable-value exp env)
            (if (quoted? exp) (text-of-quotation exp)
                (if (assignment? exp) (eval-assignment exp env)
                    (if (definition? exp) (eval-definition exp env)
                        (if (if? exp) (eval-if exp env)
                            (if (lambda? exp) (make-procedure (lambda-parameters exp)
                                                              (lambda-body exp)
                                                              env)
                                (if (begin? exp) (eval-sequence (begin-actions exp)
                                                                env)
                                    (if (application? exp) (my-apply (eval (operator exp) env)
                                                                     (list-of-values (operands exp) env))
                                        (error "eval: Unknown expression type:" exp))))))))))))

;; Uncomment this line to run the REPL.
;; (driver-loop)

;; This benchmark times calculating (fib 6) many times.

(eval '(define (fib n)
         (if (< n 2)
             n
             (+ (fib (- n 1))
                (fib (- n 2)))))
      the-global-environment)

'(eval '(fib 6) the-global-environment)
