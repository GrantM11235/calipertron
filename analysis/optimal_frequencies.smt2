(define-const kilo Int 1000)
(define-const mega Int 1000000)


(declare-const sampling-frequency Real)
(declare-const sampling-period Real)
(assert (= sampling-frequency (/ 1 sampling-period)))

(declare-const adc-clock-cycles-per-sample Real)
(declare-const adc-clock-frequency Real)


(declare-const fft-n Int)
(assert (or (= fft-n 64)
            (= fft-n 128)
            (= fft-n 256)
            (= fft-n 512)
            (= fft-n 1024)))

(declare-const fft-bin-resolution Real)
(assert (= fft-bin-resolution (/ sampling-frequency fft-n)))


(declare-const signal-bin-idx Int)
(assert (<= 0 signal-bin-idx (- fft-n 1)))


;;;;;;;;;;;;;;;;;;;;;;;;
;; PDM signal emission

(define-const pdm-samples-n Int 132)
(declare-const pdm-signal-frequency Real)
(declare-const timer-prescaler Int)
(declare-const timer-clock-frequency Real)
(declare-const timer-tick-frequency Real)
(declare-const timer-update-frequency Real)
(declare-const timer-auto-reload-register Int)

(assert (= timer-clock-frequency (* 72 mega)))
(assert (< 0 timer-auto-reload-register (^ 2 16)))
(assert (= timer-tick-frequency (/ timer-clock-frequency timer-prescaler)))
(assert (= timer-update-frequency (/ timer-tick-frequency timer-auto-reload-register)))
(assert (= pdm-signal-frequency (/ timer-update-frequency pdm-samples-n)))

(assert (<= 1 timer-prescaler (^ 2 16)))


;; loss function is how far our emitted signal frequency is from an FFT bin center
(declare-const loss Real)
(assert (= loss (^ (- (* signal-bin-idx fft-bin-resolution) pdm-signal-frequency)
                 2)))


;;;;;;;;;;;;;
;; stm32f103
(assert (= adc-clock-frequency (* 12 mega)))
(define-const adc-sample-overhead-cycles Real 12.5)

;; 11.12.4 ADC sample time register 1 (ADC_SMPR1)
(assert (or (= adc-clock-cycles-per-sample 1.5)
            (= adc-clock-cycles-per-sample 7.5)
            (= adc-clock-cycles-per-sample 13.5)
            (= adc-clock-cycles-per-sample 28.5)
            (= adc-clock-cycles-per-sample 41.5)
            (= adc-clock-cycles-per-sample 55.5)
            (= adc-clock-cycles-per-sample 71.5)
            (= adc-clock-cycles-per-sample 239.5)))

(assert (= sampling-period (/ (+ adc-clock-cycles-per-sample
                                 adc-sample-overhead-cycles)
                              adc-clock-frequency)))


;; lets have some extra margin above the nyquyst limit
(assert (> sampling-frequency (* 4 pdm-signal-frequency)))

;; TODO: make sure line noise isn't in the same bin as our signal


(assert (= adc-clock-cycles-per-sample 239.5))

(minimize loss)

(set-option :pp.decimal true)
(check-sat)
(get-model)

;;(eval sampling-frequency)

