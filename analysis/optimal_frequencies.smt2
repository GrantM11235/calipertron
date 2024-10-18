(define-const kilo Int 1000)
(define-const mega Int 1000000)


(declare-const sampling-frequency Real)
(declare-const sampling-period Real)
(assert (= sampling-frequency (/ 1 sampling-period)))

(declare-const adc-clock-cycles-per-sample Real)
(declare-const adc-clock-frequency Real)


(declare-const fft-n Int)
(assert (or
         ;; (= fft-n 64)
         ;; (= fft-n 128)
         (= fft-n 256)
         ;; (= fft-n 512)
         ;;(= fft-n 1024)
         ))

(declare-const fft-bin-resolution Real)
(assert (= fft-bin-resolution (/ sampling-frequency fft-n)))


(declare-const signal-bin-idx Int)

;; real signal, so it should be in the first half of the fft bins
(assert (<= 0 signal-bin-idx (/ fft-n 2)))


;;;;;;;;;;;;;;;;;;;;;;;;
;; PDM signal emission

(define-const pdm-samples-n Int 132)
(declare-const pdm-signal-frequency Real)

(assert (< 0 pdm-signal-frequency (* 72 mega)))

;; loss function is how far our emitted signal frequency is from an fft bin's
(declare-const loss Real)
(assert (= loss (^ (- (* signal-bin-idx fft-bin-resolution) pdm-signal-frequency)
                 2)))

;; assuming line noise is in the first bin, make sure our signal isn't
(assert (< 1 (/ pdm-signal-frequency fft-bin-resolution)))

;; lets have some extra margin above the nyquyst limit
(assert (> sampling-frequency (* 4 pdm-signal-frequency)))



;;;;;;;;;;;;;
;; stm32f103

(assert (= adc-clock-frequency (* 12 mega)))
(define-const adc-sample-overhead-cycles Real 12.5)

;; 11.12.4 ADC sample time register 1 (ADC_SMPR1)
(assert (or
         ;; (= adc-clock-cycles-per-sample 1.5)
         ;; (= adc-clock-cycles-per-sample 7.5)
         ;; (= adc-clock-cycles-per-sample 13.5)
         ;; (= adc-clock-cycles-per-sample 28.5)
         ;; (= adc-clock-cycles-per-sample 41.5)
         ;; (= adc-clock-cycles-per-sample 55.5)
         ;; (= adc-clock-cycles-per-sample 71.5)
         (= adc-clock-cycles-per-sample 239.5)))

(assert (= sampling-period (/ (+ adc-clock-cycles-per-sample
                                 adc-sample-overhead-cycles)
                              adc-clock-frequency)))




(minimize loss)

(set-option :pp.decimal true)
(check-sat)
(get-model)

;;(eval sampling-frequency)



;; (
;;   (define-fun fft-n () Int
;;     128)
;;   (define-fun mega () Int
;;     1000000)
;;   (define-fun signal-bin-idx () Int
;;     1)
;;   (define-fun pdm-signal-frequency () Real
;;     433.0)
;;   (define-fun pdm-samples-n () Int
;;     132)
;;   (define-fun kilo () Int
;;     1000)
;;   (define-fun adc-sample-overhead-cycles () Real
;;     12.5)
;;   (define-fun fft-bin-resolution () Real
;;     372.0238095238?)
;;   (define-fun sampling-frequency () Real
;;     47619.0476190476?)
;;   (define-fun sampling-period () Real
;;     0.000021)
;;   (define-fun adc-clock-frequency () Real
;;     12000000.0)
;;   (define-fun adc-clock-cycles-per-sample () Real
;;     239.5)
;;   (define-fun loss () Real
;;     3718.0958049886?)
;;   (define-fun /0 ((x!0 Real) (x!1 Real)) Real
;;     (ite (and (= x!0 60.0) (= x!1 372.0238095238?)) 0.16128
;;     (ite (and (= x!0 433.0) (= x!1 372.0238095238?)) 1.163904
;;       372.0238095238?)))
;; )
