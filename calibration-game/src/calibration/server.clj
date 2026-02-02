(ns calibration.server
  "Calibration Game Server - Train your market intuition.

   The calibration game presents historical price data and asks you to:
   1. Predict the next price movement (direction + magnitude)
   2. Assign a confidence level (50-99%)
   3. Over time, your predictions are scored for calibration

   Good calibration means: when you say 70% confident, you're right ~70% of the time.

   This connects to the arbitragefx hypothesis ledger to track your evolving beliefs."
  (:require [ring.adapter.jetty :as jetty]
            [ring.middleware.json :refer [wrap-json-response wrap-json-body]]
            [ring.util.response :as resp]
            [cheshire.core :as json]
            [clojure.java.io :as io]
            [clojure.string :as str])
  (:gen-class))

;; =============================================================================
;; Data Loading
;; =============================================================================

(def data-path "/home/uprootiny/Shevat/arbitragefx/data/btc_1h_180d.csv")

(defn parse-csv-row [line]
  (let [parts (str/split line #",")]
    (when (>= (count parts) 6)
      {:ts (Long/parseLong (nth parts 0))
       :o (Double/parseDouble (nth parts 1))
       :h (Double/parseDouble (nth parts 2))
       :l (Double/parseDouble (nth parts 3))
       :c (Double/parseDouble (nth parts 4))
       :v (Double/parseDouble (nth parts 5))})))

(defn load-market-data []
  (try
    (with-open [rdr (io/reader data-path)]
      (->> (line-seq rdr)
           (drop 1)  ; Skip header
           (map parse-csv-row)
           (filter some?)
           (vec)))
    (catch Exception e
      (println "Error loading data:" (.getMessage e))
      [])))

;; =============================================================================
;; Game State
;; =============================================================================

(def game-state (atom {:players {}
                       :current-challenges {}
                       :leaderboard []}))

(defn generate-challenge
  "Generate a calibration challenge from market data.
   Shows N bars of history, asks to predict the next bar."
  [data history-bars]
  (let [max-start (- (count data) history-bars 1)
        start-idx (rand-int max-start)
        history (subvec data start-idx (+ start-idx history-bars))
        answer (nth data (+ start-idx history-bars))
        last-close (:c (last history))
        actual-change (/ (- (:c answer) last-close) last-close)]
    {:id (str (System/currentTimeMillis))
     :history history
     :last-close last-close
     :answer {:price (:c answer)
              :change-pct (* 100 actual-change)
              :direction (if (pos? actual-change) :up :down)}}))

(defn score-prediction
  "Score a prediction against actual outcome.
   Returns calibration metrics."
  [prediction actual]
  (let [predicted-dir (:direction prediction)
        actual-dir (:direction actual)
        confidence (/ (:confidence prediction) 100.0)
        correct? (= predicted-dir actual-dir)

        ;; Brier score: (confidence - outcome)^2
        ;; Lower is better, 0 is perfect
        outcome (if correct? 1.0 0.0)
        brier (Math/pow (- confidence outcome) 2)

        ;; Calibration contribution
        ;; If you said 70% and were right, that's good calibration
        ;; If you said 70% and were wrong, that's poor calibration
        calibration-bucket (int (* confidence 10))]

    {:correct correct?
     :confidence confidence
     :brier-score brier
     :calibration-bucket calibration-bucket
     :actual-change (:change-pct actual)
     :predicted-direction predicted-dir}))

(defn update-player-stats [player-id result]
  (swap! game-state update-in [:players player-id]
         (fn [stats]
           (let [stats (or stats {:predictions []
                                  :calibration-buckets {}
                                  :total-brier 0.0
                                  :total-predictions 0})]
             (-> stats
                 (update :predictions conj result)
                 (update :total-brier + (:brier-score result))
                 (update :total-predictions inc)
                 (update-in [:calibration-buckets (:calibration-bucket result)]
                            (fn [bucket]
                              (let [b (or bucket {:total 0 :correct 0})]
                                (-> b
                                    (update :total inc)
                                    (update :correct (if (:correct result) inc identity)))))))))))

;; =============================================================================
;; API Handlers
;; =============================================================================

(defn handler [request]
  (let [uri (:uri request)
        method (:request-method request)
        data (load-market-data)]

    (cond
      ;; Get a new challenge
      (and (= method :get) (= uri "/api/challenge"))
      (let [challenge (generate-challenge data 24)]  ; 24 hours of history
        (swap! game-state assoc-in [:current-challenges (:id challenge)]
               (:answer challenge))
        (resp/response {:challenge (dissoc challenge :answer)
                        :instructions "Predict: will the next candle close UP or DOWN? How confident are you (50-99%)?"
                        :reflection "Consider: What patterns do you see? What is your gut telling you? What would the hypothesis ledger suggest?"}))

      ;; Submit prediction
      (and (= method :post) (= uri "/api/predict"))
      (let [body (:body request)
            challenge-id (:challenge-id body)
            prediction {:direction (keyword (:direction body))
                        :confidence (:confidence body)}
            actual (get-in @game-state [:current-challenges challenge-id])]
        (if actual
          (let [result (score-prediction prediction actual)
                player-id (or (:player-id body) "anonymous")]
            (update-player-stats player-id result)
            (swap! game-state update :current-challenges dissoc challenge-id)
            (resp/response {:result result
                            :actual actual
                            :feedback (cond
                                        (and (:correct result) (> (:confidence result) 0.8))
                                        "Excellent! High confidence, correct prediction."

                                        (and (not (:correct result)) (> (:confidence result) 0.8))
                                        "Overconfident. The market humbled you this time."

                                        (and (:correct result) (< (:confidence result) 0.6))
                                        "Correct but underconfident. Trust your analysis more."

                                        :else
                                        "Keep practicing. Calibration is a skill that develops with experience.")}))
          (resp/bad-request {:error "Challenge not found or expired"})))

      ;; Get player stats
      (and (= method :get) (str/starts-with? uri "/api/stats/"))
      (let [player-id (subs uri 11)
            stats (get-in @game-state [:players player-id])]
        (if stats
          (let [avg-brier (/ (:total-brier stats) (max 1 (:total-predictions stats)))
                calibration-curve (map (fn [[bucket data]]
                                         {:confidence-range [(* bucket 10) (+ (* bucket 10) 10)]
                                          :actual-accuracy (if (pos? (:total data))
                                                             (/ (:correct data) (:total data))
                                                             0)
                                          :samples (:total data)})
                                       (:calibration-buckets stats))]
            (resp/response {:total-predictions (:total-predictions stats)
                            :average-brier-score avg-brier
                            :calibration-curve (sort-by #(first (:confidence-range %)) calibration-curve)
                            :interpretation (cond
                                              (< avg-brier 0.1) "Excellent calibration!"
                                              (< avg-brier 0.2) "Good calibration, room for improvement"
                                              (< avg-brier 0.3) "Moderate calibration"
                                              :else "Poor calibration - reconsider your confidence levels")}))
          (resp/response {:message "No stats yet. Start playing!"})))

      ;; Market data summary
      (and (= method :get) (= uri "/api/market-summary"))
      (let [last-100 (take-last 100 data)
            first-price (:c (first last-100))
            last-price (:c (last last-100))
            change-pct (* 100 (/ (- last-price first-price) first-price))]
        (resp/response {:bars-available (count data)
                        :recent-trend (if (pos? change-pct) :bullish :bearish)
                        :change-100-bars (str (format "%.2f" change-pct) "%")
                        :current-price last-price}))

      ;; Health
      (and (= method :get) (= uri "/api/health"))
      (resp/response {:status "ok"
                      :game "calibration"
                      :data-loaded (count data)})

      ;; Static files / index
      (= uri "/")
      (-> (resp/response (slurp (io/resource "public/calibration.html")))
          (resp/content-type "text/html"))

      :else
      (resp/not-found {:error "Not found"}))))

(defn wrap-cors [handler]
  (fn [request]
    (-> (handler request)
        (resp/header "Access-Control-Allow-Origin" "*")
        (resp/header "Access-Control-Allow-Methods" "GET, POST, OPTIONS")
        (resp/header "Access-Control-Allow-Headers" "Content-Type"))))

(def app
  (-> handler
      wrap-cors
      wrap-json-response
      (wrap-json-body {:keywords? true})))

(defn -main [& args]
  (let [port (+ 40000 (rand-int 10000))]
    (println)
    (println "=== CALIBRATION GAME ===")
    (println)
    (println "Train your market intuition through deliberate practice.")
    (println "The game presents historical price data and asks you to predict")
    (println "the next movement with a confidence level.")
    (println)
    (println "Over time, you'll develop calibrated confidence -")
    (println "knowing when you know, and when you don't.")
    (println)
    (println (str "Server running at http://localhost:" port))
    (println "API endpoints:")
    (println "  GET  /api/challenge      - Get a new prediction challenge")
    (println "  POST /api/predict        - Submit your prediction")
    (println "  GET  /api/stats/:player  - View your calibration stats")
    (println)
    (jetty/run-jetty app {:port port :join? true})))
