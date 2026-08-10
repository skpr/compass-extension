<?php
// Exercises every dispatch path in observer.rs: the Drupal CacheableMetadata
// names, the PDO seam, and plenty of ordinary functions.

namespace Drupal\Core\Cache {
    class CacheableMetadata {
        public $cacheMaxAge = 42;
        public $cacheTags = ['node:1', 'node:2'];
        public $cacheContexts = ['user', 'url.path'];
        public static function createFromObject($object) { return new self(); }
        public static function createFromRenderArray(array $array) { return new self(); }
    }
}

namespace Compass\Test {
    use Drupal\Core\Cache\CacheableMetadata;
    use PDO;

    // Same bare names as the probe targets but the wrong class - must be
    // classified Generic, not matched.
    class Decoy {
        public function execute() { return 1; }
        public function query() { return 2; }
        public function exec() { return 3; }
        public static function createFromObject($o) { return 4; }
    }

    class Fixture {
        private PDO $db;

        public function __construct() {
            $this->db = new PDO('sqlite::memory:');
            $this->db->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
        }

        // Called as a METHOD on purpose: get_function_or_method_name() only
        // allocates a fresh zend_string for methods, which is the case that
        // triggered the use-after-free.
        public function drupalProbes(): int {
            $a = CacheableMetadata::createFromObject(new Decoy());
            $b = CacheableMetadata::createFromRenderArray(['#markup' => 'x']);
            return $a->cacheMaxAge + $b->cacheMaxAge;
        }

        public function dbProbes(): int {
            // PDO::exec - SQL is parameter 0
            $this->db->exec('CREATE TABLE node (nid INTEGER, title TEXT)');

            // PDOStatement::execute - SQL is on $this->queryString
            $insert = $this->db->prepare('INSERT INTO node (nid, title) VALUES (:nid, :title)');
            for ($i = 1; $i <= 25; $i++) {
                $insert->execute([':nid' => $i, ':title' => "node $i"]);
            }

            // Unparameterised literals - these should collapse to one sql_id
            $seen = 0;
            for ($i = 1; $i <= 10; $i++) {
                $stmt = $this->db->query("SELECT title FROM node WHERE nid = $i");
                $seen += count($stmt->fetchAll(PDO::FETCH_ASSOC));
            }

            $select = $this->db->prepare('SELECT COUNT(*) FROM node');
            $select->execute();
            return (int) $select->fetchColumn() + $seen;
        }

        public function churn(int $n): int {
            $total = 0;
            $decoy = new Decoy();
            for ($i = 0; $i < $n; $i++) {
                $total += $decoy->execute() + $decoy->query() + $decoy->exec();
                $total += strlen((string) $i) + Decoy::createFromObject($i);
            }
            return $total;
        }
    }

    $f = new Fixture();
    printf("drupal=%d db=%d churn=%d\n", $f->drupalProbes(), $f->dbProbes(), $f->churn(2000));
    echo "OK\n";
}
