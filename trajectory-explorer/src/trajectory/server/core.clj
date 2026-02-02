(ns trajectory.server.core
  (:require [ring.adapter.jetty :as jetty]
            [ring.middleware.json :refer [wrap-json-response wrap-json-body]]
            [ring.middleware.cors :refer [wrap-cors]]
            [ring.util.response :as resp]
            [cheshire.core :as json]
            [clojure.java.shell :as shell]
            [clojure.string :as str]
            [clojure.java.io :as io])
  (:gen-class))

(def repo-path "/home/uprootiny/Shevat/arbitragefx")

;; =============================================================================
;; Git History
;; =============================================================================

(defn parse-commit-line [line]
  (let [[hash ts author subject] (str/split line #"\|" 4)]
    {:hash (str/trim (or hash ""))
     :timestamp (try (Long/parseLong (str/trim (or ts "0")))
                     (catch Exception _ 0))
     :author (str/trim (or author ""))
     :subject (str/trim (or subject ""))}))

(defn get-git-log []
  (let [result (shell/sh "git" "log" "--format=%H|%at|%an|%s" "-100"
                         :dir repo-path)]
    (if (= 0 (:exit result))
      (->> (str/split-lines (:out result))
           (filter #(not (str/blank? %)))
           (map parse-commit-line)
           (vec))
      [])))

(defn get-commit-files [hash]
  (let [result (shell/sh "git" "diff-tree" "--no-commit-id" "--name-status" "-r" hash
                         :dir repo-path)]
    (if (= 0 (:exit result))
      (->> (str/split-lines (:out result))
           (filter #(not (str/blank? %)))
           (map #(let [[status path] (str/split % #"\t" 2)]
                   {:status (case status "A" :added "M" :modified "D" :deleted :other)
                    :path path}))
           (vec))
      [])))

(defn get-branches []
  (let [result (shell/sh "git" "branch" "-a" "--format=%(refname:short)"
                         :dir repo-path)]
    (if (= 0 (:exit result))
      (->> (str/split-lines (:out result))
           (filter #(not (str/blank? %)))
           (vec))
      [])))

;; =============================================================================
;; Hypothesis Ledger
;; =============================================================================

(defn load-hypothesis-ledger []
  (let [path (str repo-path "/data/hypothesis_ledger.json")]
    (if (.exists (io/file path))
      (json/parse-string (slurp path) true)
      {:hypotheses {} :evidence []})))

;; =============================================================================
;; Prompt History (from Claude session files)
;; =============================================================================

(defn find-session-files []
  (let [claude-dir (io/file "/home/uprootiny/.claude/projects/-home-uprootiny-Shevat-arbitragefx")]
    (if (.exists claude-dir)
      (->> (.listFiles claude-dir)
           (filter #(.endsWith (.getName %) ".jsonl"))
           (map #(.getAbsolutePath %))
           (vec))
      [])))

(defn parse-jsonl-entry [line]
  (try
    (json/parse-string line true)
    (catch Exception _ nil)))

(defn extract-prompts [session-file]
  (try
    (->> (line-seq (io/reader session-file))
         (map parse-jsonl-entry)
         (filter some?)
         (filter #(= "human" (:type %)))
         (map #(select-keys % [:message :timestamp]))
         (take 50)
         (vec))
    (catch Exception _ [])))

(defn get-prompt-history []
  (let [sessions (find-session-files)]
    (->> sessions
         (take 5)
         (mapcat extract-prompts)
         (sort-by :timestamp)
         (vec))))

;; =============================================================================
;; Trajectory Extrapolation
;; =============================================================================

(defn classify-commit [{:keys [subject]}]
  (cond
    (re-find #"(?i)hypothesis|research|evidence" subject) :research
    (re-find #"(?i)signal|indicator|strategy" subject) :strategy
    (re-find #"(?i)test|verify|validation" subject) :testing
    (re-find #"(?i)fix|bug|error" subject) :bugfix
    (re-find #"(?i)refactor|clean|improve" subject) :refactor
    (re-find #"(?i)doc|readme|comment" subject) :documentation
    (re-find #"(?i)add|implement|create" subject) :feature
    :else :other))

(defn analyze-trajectory [commits]
  (let [classified (map #(assoc % :category (classify-commit %)) commits)
        by-category (group-by :category classified)
        recent (take 10 commits)]
    {:total-commits (count commits)
     :by-category (into {} (map (fn [[k v]] [k (count v)]) by-category))
     :recent-focus (->> recent
                        (map :category)
                        (frequencies)
                        (sort-by val >)
                        (first)
                        (first))
     :commits classified}))

(defn suggest-next-steps [trajectory ledger]
  (let [focus (:recent-focus trajectory)
        hypotheses (:hypotheses ledger)
        evidence (:evidence ledger)
        untested (->> hypotheses
                      (filter (fn [[_ h]] (= "Proposed" (:status h))))
                      (keys))]
    (concat
     ;; Based on hypothesis status
     (when (seq untested)
       [{:type :test
         :priority :high
         :description (str "Test untested hypotheses: " (str/join ", " (take 3 untested)))}])

     ;; Based on recent focus
     (case focus
       :research [{:type :implement
                   :priority :medium
                   :description "Implement supported hypotheses as live strategies"}]
       :strategy [{:type :test
                   :priority :high
                   :description "Run backtests on new strategies"}]
       :testing [{:type :refine
                  :priority :medium
                  :description "Refine strategies based on test results"}]
       [{:type :explore
         :priority :low
         :description "Continue current development trajectory"}])

     ;; Regime coverage
     (let [tested-regimes (->> evidence
                               (map :regime)
                               (set))]
       (when (not (contains? tested-regimes "StrongBull"))
         [{:type :data
           :priority :medium
           :description "Acquire bull market data for regime coverage"}])))))

(defn extrapolate-futures [trajectory]
  ;; Generate possible future development paths
  [{:id :path-a
    :name "Production Deployment"
    :steps ["Finalize supported strategies"
            "Implement regime detection"
            "Add risk management layer"
            "Paper trading validation"
            "Gradual live deployment"]
    :probability 0.4}
   {:id :path-b
    :name "Research Deepening"
    :steps ["Test in more market regimes"
            "Develop new indicator combinations"
            "Machine learning signal enhancement"
            "Cross-asset correlation strategies"]
    :probability 0.3}
   {:id :path-c
    :name "Infrastructure Hardening"
    :steps ["Improve logging and monitoring"
            "Add circuit breakers"
            "Implement position reconciliation"
            "Multi-exchange support"]
    :probability 0.3}])

;; =============================================================================
;; API Routes
;; =============================================================================

(defn api-routes [request]
  (let [uri (:uri request)
        method (:request-method request)]
    (cond
      ;; Git history
      (and (= method :get) (= uri "/api/commits"))
      (resp/response {:commits (get-git-log)})

      (and (= method :get) (str/starts-with? uri "/api/commit/"))
      (let [hash (subs uri 12)]
        (resp/response {:files (get-commit-files hash)}))

      (and (= method :get) (= uri "/api/branches"))
      (resp/response {:branches (get-branches)})

      ;; Hypothesis ledger
      (and (= method :get) (= uri "/api/ledger"))
      (resp/response (load-hypothesis-ledger))

      ;; Prompt history
      (and (= method :get) (= uri "/api/prompts"))
      (resp/response {:prompts (get-prompt-history)})

      ;; Trajectory analysis
      (and (= method :get) (= uri "/api/trajectory"))
      (let [commits (get-git-log)
            ledger (load-hypothesis-ledger)
            trajectory (analyze-trajectory commits)]
        (resp/response {:trajectory trajectory
                        :suggestions (suggest-next-steps trajectory ledger)
                        :futures (extrapolate-futures trajectory)}))

      ;; Health check
      (and (= method :get) (= uri "/api/health"))
      (resp/response {:status "ok" :repo repo-path})

      ;; Static files
      (= uri "/")
      (-> (resp/resource-response "public/index.html")
          (resp/content-type "text/html"))

      (str/starts-with? uri "/js/")
      (resp/resource-response (str "public" uri))

      :else
      (resp/not-found {:error "Not found"}))))

(defn wrap-cors-headers [handler]
  (fn [request]
    (let [response (handler request)]
      (-> response
          (resp/header "Access-Control-Allow-Origin" "*")
          (resp/header "Access-Control-Allow-Methods" "GET, POST, OPTIONS")
          (resp/header "Access-Control-Allow-Headers" "Content-Type")))))

(def app
  (-> api-routes
      wrap-cors-headers
      wrap-json-response
      (wrap-json-body {:keywords? true})))

(defn -main [& args]
  (let [port (+ 40000 (rand-int 10000))]  ;; Random high port
    (println (str "Starting Trajectory Explorer on http://localhost:" port))
    (println (str "API available at http://localhost:" port "/api/"))
    (jetty/run-jetty app {:port port :join? true})))
