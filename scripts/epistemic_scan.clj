#!/usr/bin/env bb
;; Epistemic Scanner for arbitragefx
;; Analyzes codebase and generates epistemic annotations
;;
;; Run: bb scripts/epistemic_scan.clj

(require '[clojure.java.io :as io]
         '[clojure.string :as str]
         '[clojure.edn :as edn])

(def project-root ".")

;; Patterns that indicate epistemic levels
(def level-patterns
  {:verified [#"#\[test\]"
              #"assert!"
              #"assert_eq!"
              #"debug_assert!"
              #"#\[cfg\(test\)\]"]
   :invariant [#"// INVARIANT:"
               #"/// Invariant:"
               #"#\[must_use\]"
               #"assert!\(.+\)"  ; runtime assertions
               #"unreachable!"]
   :assumed [#"// TODO:"
             #"// FIXME:"
             #"// ASSUMPTION:"
             #"unwrap\(\)"       ; assumes success
             #"expect\("]
   :inferred [#"// Derived from"
              #"// Calculated:"
              #"// Based on"]})

(defn count-patterns [content patterns]
  (reduce (fn [acc pattern]
            (+ acc (clojure.core/count (re-seq pattern content))))
          0 patterns))

(defn analyze-file [path]
  (try
    (let [content (slurp path)
          lines (count (str/split-lines content))
          verified (count-patterns content (:verified level-patterns))
          invariant (count-patterns content (:invariant level-patterns))
          assumed (count-patterns content (:assumed level-patterns))
          test-coverage (/ (+ verified invariant) (max lines 1))]
      {:path (str path)
       :lines lines
       :verified verified
       :invariant invariant
       :assumed assumed
       :level (cond
                (> test-coverage 0.1) :verified
                (> invariant 3) :invariant
                (> assumed 5) :assumed
                :else :asserted)
       :confidence (min 1.0 (* test-coverage 10))})
    (catch Exception e
      {:path (str path) :error (str e)})))

(defn scan-directory [dir pattern]
  (let [dir-file (io/file dir)]
    (if (.exists dir-file)
      (->> (file-seq dir-file)
           (filter #(.isFile %))
           (filter #(re-matches pattern (.getName %)))
           (map analyze-file)
           (remove :error))
      (do (println "Directory not found:" dir) []))))

(defn generate-report [results]
  (let [by-level (group-by :level results)
        total (count results)
        verified (count (by-level :verified))
        coverage (* 100.0 (/ verified (max total 1)))]
    {:summary {:total-files total
               :verified verified
               :coverage (format "%.1f%%" coverage)}
     :by-level (into {} (map (fn [[k v]] [k (count v)]) by-level))
     :highest-confidence (take 5 (reverse (sort-by :confidence results)))
     :lowest-confidence (take 5 (sort-by :confidence results))}))

;; Main
(println "Epistemic Scanner for arbitragefx")
(println "================================")
(println)

(let [rust-files (scan-directory "src" #".*\.rs")
      report (generate-report rust-files)]

  (println "Summary:")
  (println (str "  Total files: " (-> report :summary :total-files)))
  (println (str "  Verified: " (-> report :summary :verified)))
  (println (str "  Coverage: " (-> report :summary :coverage)))
  (println)

  (println "By Level:")
  (doseq [[level count] (:by-level report)]
    (println (str "  " (name level) ": " count)))
  (println)

  (println "Highest Confidence:")
  (doseq [f (:highest-confidence report)]
    (println (str "  " (:path f) " (" (format "%.2f" (double (:confidence f))) ")")))
  (println)

  (println "Needs Attention:")
  (doseq [f (:lowest-confidence report)]
    (println (str "  " (:path f) " (" (format "%.2f" (double (:confidence f))) ")")))

  ;; Write EDN output
  (spit "epistemic_state.edn" (pr-str report))
  (println)
  (println "Full report written to: epistemic_state.edn"))
