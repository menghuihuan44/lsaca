// lisp.ls - a small Lisp interpreter written in lsaca.
//
// The program is deterministic: it evaluates an embedded script, prints the
// values produced by special form (print ...), then reports bookkeeping
// counts.  An optional file argument evaluates a script from disk instead of
// the embedded one.  examples/programs/lisp.c mirrors this interpreter and
// must produce byte-identical output.
struct LispError {
    message: String,
}
impl LispError {
    fn new(message: &str) -> LispError {
        LispError { message: format!("{}", message) }
    }
    fn describe(&self) -> String {
        self.message.clone()
    }
}
enum Sexp {
    Num(i64),
    Bool(bool),
    Sym(String),
    Str(String),
    List(Vec<Sexp>),
    Lambda(Vec<String>, Vec<Sexp>, Vec<Sexp>),
}
struct Stats {
    expressions: usize,
    prints: usize,
}
impl Stats {
    fn new() -> Stats {
        Stats { expressions: 0, prints: 0 }
    }
}
struct Parser {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
}
impl Parser {
    fn new(text: &str) -> Parser {
        Parser { chars: text.chars(), index: 0, line: 1, column: 1 }
    }
    fn peek(&self) -> char {
        if self.index >= self.chars.len() { return '\0'; }
        return self.chars[self.index];
    }
    fn advance(&mut self) -> char {
        let ch = self.peek();
        if ch == '\0' { return ch; }
        self.index += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        return ch;
    }
    fn peek_next(&self) -> char {
        if self.index + 1 >= self.chars.len() { return '\0'; }
        return self.chars[self.index + 1];
    }
    fn skip_space(&mut self) {
        loop {
            let ch = self.peek();
            if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                self.advance();
                continue;
            }
            if ch == ';' {
                while self.peek() != '\n' && self.peek() != '\0' {
                    self.advance();
                }
                continue;
            }
            break;
        }
    }
    fn error(&self, message: &str) -> LispError {
        LispError { message: format!("{} at line {} column {}", message, self.line, self.column) }
    }
    fn parse_all(&mut self) -> Result<Vec<Sexp>, LispError> {
        let mut expressions: Vec<Sexp> = Vec::new();
        loop {
            self.skip_space();
            if self.peek() == '\0' {
                break;
            }
            expressions.push(self.parse_expr()?);
        }
        return Ok(expressions);
    }
    fn is_delimiter(ch: char) -> bool { return ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' || ch == '(' || ch == ')' || ch == ';'; }
    fn parse_expr(&mut self) -> Result<Sexp, LispError> {
        self.skip_space();
        let ch = self.peek();
        if ch == '\0' { return Err(self.error("unexpected end of input")); }
        if ch == '(' {
            self.advance();
            return self.parse_list();
        }
        if ch == ')' { return Err(self.error("unexpected ')'")); }
        if ch == '\'' {
            self.advance();
            let inner = self.parse_expr()?;
            return Ok(Sexp::List(vec![Sexp::Sym(String::from("quote")), inner]));
        }
        if ch == '"' { return self.parse_text(); }
        if ch >= '0' && ch <= '9' { return self.parse_number(); }
        if ch == '-' {
            let next = self.peek_next();
            if next >= '0' && next <= '9' { return self.parse_number(); }
            return self.parse_symbol();
        }
        return self.parse_symbol();
    }
    fn parse_list(&mut self) -> Result<Sexp, LispError> {
        let mut items: Vec<Sexp> = Vec::new();
        loop {
            self.skip_space();
            let ch = self.peek();
            if ch == '\0' { return Err(self.error("expected ')' but reached end of input")); }
            if ch == ')' {
                self.advance();
                return Ok(Sexp::List(items));
            }
            items.push(self.parse_expr()?);
        }
    }
    fn parse_text(&mut self) -> Result<Sexp, LispError> {
        self.advance();
        let mut text = String::new();
        loop {
            let ch = self.advance();
            if ch == '\0' { return Err(self.error("unterminated string literal")); }
            if ch == '"' { return Ok(Sexp::Str(text)); }
            if ch == '\\' {
                let escaped = self.advance();
                if escaped == '\0' { return Err(self.error("unterminated string literal")); }
                if escaped == 'n' {
                    text.push('\n');
                } else if escaped == 't' {
                    text.push('\t');
                } else {
                    text.push(escaped);
                }
                continue;
            }
            text.push(ch);
        }
    }
    fn parse_number(&mut self) -> Result<Sexp, LispError> {
        let start = self.index;
        if self.peek() == '-' {
            self.advance();
        }
        while self.peek() >= '0' && self.peek() <= '9' {
            self.advance();
        }
        if Parser::is_delimiter(self.peek()) == false && self.peek() != '\0' {
            while Parser::is_delimiter(self.peek()) == false {
                self.advance();
            }
            return Err(self.error("invalid number literal"));
        }
        let text = slice_chars(&self.chars, start, self.index);
        if text == "-" { return Err(self.error("invalid number literal")); }
        match text.parse_i64() {
            Ok(value) => return Ok(Sexp::Num(value)),
            Err(_) => return Err(self.error("invalid number literal")),
        }
    }
    fn parse_symbol(&mut self) -> Result<Sexp, LispError> {
        let start = self.index;
        while Parser::is_delimiter(self.peek()) == false && self.peek() != '\0' {
            self.advance();
        }
        let text = slice_chars(&self.chars, start, self.index);
        if text == "#t" { return Ok(Sexp::Bool(true)); }
        if text == "#f" { return Ok(Sexp::Bool(false)); }
        if text.len() == 0 { return Err(self.error("empty symbol")); }
        return Ok(Sexp::Sym(text));
    }
}
fn slice_chars(chars: &Vec<char>, start: usize, end: usize) -> String {
    let mut out = String::new();
    let mut index = start;
    while index < end && index < chars.len() {
        out.push(chars[index]);
        index += 1;
    }
    return out;
}
fn text_equals(left: &str, right: &str) -> bool {
    let left_owned = format!("{}", left);
    let right_owned = format!("{}", right);
    return left_owned == right_owned;
}
fn quote_text(text: &str) -> String {
    let mut out = String::from("\"");
    let chars = text.chars();
    let mut index: usize = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '"' {
            out.push_str("\\\"");
        } else if ch == '\\' {
            out.push_str("\\\\");
        } else if ch == '\n' {
            out.push_str("\\n");
        } else if ch == '\t' {
            out.push_str("\\t");
        } else {
            out.push(ch);
        }
        index += 1;
    }
    out.push_str("\"");
    return out;
}
fn display(expr: &Sexp) -> String {
    match expr {
        Sexp::Num(number) => return format!("{}", *number),
        Sexp::Bool(flag) => {
            if *flag { return String::from("true"); }
            return String::from("false");
        }
        Sexp::Sym(name) => return name.clone(),
        Sexp::Str(text) => return quote_text(text.as_str()),
        Sexp::Lambda(params, _body, _captured) => return format!("<lambda/{}>", params.len()),
        Sexp::List(items) => {
            let mut out = String::from("(");
            let mut index: usize = 0;
            while index < items.len() {
                if index > 0 {
                    out.push(' ');
                }
                out.push_str(&display(&items[index]));
                index += 1;
            }
            out.push(')');
            return out;
        }
    }
}
fn is_truthy(expr: &Sexp) -> bool {
    match expr {
        Sexp::Bool(flag) => return *flag,
        Sexp::List(items) => return items.len() > 0,
        _ => return true,
    }
}
fn binding_pair(name: &str, value: Sexp) -> Sexp {
    let pair = vec![Sexp::Sym(format!("{}", name)), value];
    return Sexp::List(pair);
}
fn env_push(env: &mut Vec<Sexp>, name: &str, value: Sexp) {
    env.push(binding_pair(name, value));
}
fn binding_name(entry: &Sexp) -> Option<String> {
    match entry {
        Sexp::List(pair) => {
            if pair.len() == 2 {
                match &pair[0] {
                    Sexp::Sym(name) => return Some(name.clone()),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    return None;
}
fn env_lookup(env: &Vec<Sexp>, name: &str) -> Option<Sexp> {
    let mut index = env.len();
    while index > 0 {
        index -= 1;
        match &env[index] {
            Sexp::List(pair) => {
                if pair.len() == 2 {
                    match &pair[0] {
                        Sexp::Sym(key) => {
                            if text_equals(key.as_str(), name) { return Some(pair[1].clone()); }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    return None;
}
fn env_set(env: &mut Vec<Sexp>, name: &str, value: Sexp) -> bool {
    let mut index = env.len();
    while index > 0 {
        index -= 1;
        let matched = match &env[index] {
            Sexp::List(pair) => {
                if pair.len() == 2 {
                    match &pair[0] {
                        Sexp::Sym(key) => text_equals(key.as_str(), name),
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        if matched {
            env[index] = binding_pair(name, value);
            return true;
        }
    }
    return false;
}
fn expect_number(expr: &Sexp) -> Result<i64, LispError> {
    match expr {
        Sexp::Num(value) => return Ok(*value),
        _ => return Err(LispError::new(&format!("expected a number, found {}", display(expr)))),
    }
}
fn eval_args(items: &Vec<Sexp>, start: usize, env: &mut Vec<Sexp>, stats: &mut Stats) -> Result<Vec<Sexp>, LispError> {
    let mut args: Vec<Sexp> = Vec::new();
    let mut index = start;
    while index < items.len() {
        args.push(eval(&items[index], env, stats)?);
        index += 1;
    }
    return Ok(args);
}
fn builtin_arithmetic(name: &str, args: &Vec<Sexp>) -> Result<Sexp, LispError> {
    if args.len() == 0 {
        return Err(LispError::new(&format!("{} expects at least one argument", name)));
    }
    let mut total = expect_number(&args[0])?;
    if text_equals(name, "-") && args.len() == 1 { return Ok(Sexp::Num(-total)); }
    if text_equals(name, "/") && args.len() == 1 {
        if total == 0 { return Err(LispError::new("division by zero")); }
        return Ok(Sexp::Num(1 / total));
    }
    let mut index: usize = 1;
    while index < args.len() {
        let value = expect_number(&args[index])?;
        if text_equals(name, "+") {
            total = total + value;
        } else if text_equals(name, "-") {
            total = total - value;
        } else if text_equals(name, "*") {
            total = total * value;
        } else {
            if value == 0 { return Err(LispError::new("division by zero")); }
            total = total / value;
        }
        index += 1;
    }
    return Ok(Sexp::Num(total));
}
fn builtin_compare(name: &str, args: &Vec<Sexp>) -> Result<Sexp, LispError> {
    if args.len() != 2 {
        return Err(LispError::new(&format!("{} expects two arguments", name)));
    }
    let left = expect_number(&args[0])?;
    let right = expect_number(&args[1])?;
    if text_equals(name, "=") { return Ok(Sexp::Bool(left == right)); }
    if text_equals(name, "<") { return Ok(Sexp::Bool(left < right)); }
    if text_equals(name, ">") { return Ok(Sexp::Bool(left > right)); }
    if text_equals(name, "<=") { return Ok(Sexp::Bool(left <= right)); }
    return Ok(Sexp::Bool(left >= right));
}
fn builtin_list(name: &str, args: &Vec<Sexp>) -> Result<Sexp, LispError> {
    if text_equals(name, "list") { return Ok(Sexp::List(args.clone())); }
    if text_equals(name, "car") || text_equals(name, "cdr") {
        if args.len() != 1 {
            return Err(LispError::new(&format!("{} expects one argument", name)));
        }
        match &args[0] {
            Sexp::List(items) => {
                if items.len() == 0 {
                    return Err(LispError::new(&format!("{} of an empty list", name)));
                }
                if text_equals(name, "car") { return Ok(items[0].clone()); }
                let mut rest: Vec<Sexp> = Vec::new();
                let mut index: usize = 1;
                while index < items.len() {
                    rest.push(items[index].clone());
                    index += 1;
                }
                return Ok(Sexp::List(rest));
            }
            _ => return Err(LispError::new(&format!("{} expects a list", name))),
        }
    }
    if text_equals(name, "cons") {
        if args.len() != 2 { return Err(LispError::new("cons expects two arguments")); }
        let mut items = vec![args[0].clone()];
        match &args[1] {
            Sexp::List(tail) => {
                let mut index: usize = 0;
                while index < tail.len() {
                    items.push(tail[index].clone());
                    index += 1;
                }
            }
            _ => items.push(args[1].clone()),
        }
        return Ok(Sexp::List(items));
    }
    if text_equals(name, "length") {
        if args.len() != 1 { return Err(LispError::new("length expects one argument")); }
        match &args[0] {
            Sexp::List(items) => return Ok(Sexp::Num(items.len() as i64)),
            Sexp::Str(text) => return Ok(Sexp::Num(text.char_count() as i64)),
            _ => return Err(LispError::new("length expects a list or string")),
        }
    }
    if text_equals(name, "null?") {
        if args.len() != 1 { return Err(LispError::new("null? expects one argument")); }
        match &args[0] {
            Sexp::List(items) => return Ok(Sexp::Bool(items.len() == 0)),
            _ => return Ok(Sexp::Bool(false)),
        }
    }
    if text_equals(name, "number?") { return Ok(Sexp::Bool(args.len() == 1 && matches_number(&args[0]))); }
    if text_equals(name, "string?") { return Ok(Sexp::Bool(args.len() == 1 && matches_string(&args[0]))); }
    if text_equals(name, "symbol?") { return Ok(Sexp::Bool(args.len() == 1 && matches_symbol(&args[0]))); }
    if text_equals(name, "not") { return Ok(Sexp::Bool(args.len() == 1 && is_truthy(&args[0]) == false)); }
    return Err(LispError::new(&format!("unknown builtin {}", name)));
}
fn matches_number(expr: &Sexp) -> bool {
    match expr {
        Sexp::Num(_) => return true,
        _ => return false,
    }
}
fn matches_string(expr: &Sexp) -> bool {
    match expr {
        Sexp::Str(_) => return true,
        _ => return false,
    }
}
fn matches_symbol(expr: &Sexp) -> bool {
    match expr {
        Sexp::Sym(_) => return true,
        _ => return false,
    }
}
fn apply_lambda(callee: &Sexp, args: Vec<Sexp>, caller: &Vec<Sexp>, stats: &mut Stats) -> Result<Sexp, LispError> {
    match callee {
        Sexp::Lambda(params, body, captured) => {
            if args.len() != params.len() {
                return Err(LispError::new(&format!("lambda expects {} arguments, found {}", params.len(), args.len())));
            }
            let mut env = captured.clone();
            let mut outer_index: usize = 0;
            while outer_index < caller.len() {
                match binding_name(&caller[outer_index]) {
                    Some(outer_name) => {
                        let mut present = false;
                        match env_lookup(&env, outer_name.as_str()) {
                            Some(_existing) => present = true,
                            None => {}
                        }
                        if present == false {
                            env.push(caller[outer_index].clone());
                        }
                    }
                    None => {}
                }
                outer_index += 1;
            }
            let mut index: usize = 0;
            while index < params.len() {
                env.push(binding_pair(params[index].as_str(), args[index].clone()));
                index += 1;
            }
            let mut body_index: usize = 0;
            let mut result = Sexp::Bool(false);
            while body_index < body.len() {
                result = eval(&body[body_index], &mut env, stats)?;
                body_index += 1;
            }
            return Ok(result);
        }
        _ => return Err(LispError::new(&format!("cannot call {}", display(callee)))),
    }
}
fn eval(expr: &Sexp, env: &mut Vec<Sexp>, stats: &mut Stats) -> Result<Sexp, LispError> {
    stats.expressions += 1;
    match expr {
        Sexp::Num(_) | Sexp::Bool(_) | Sexp::Str(_) | Sexp::Lambda(_, _, _) => return Ok(expr.clone()),
        Sexp::Sym(name) => {
            match env_lookup(env, name.as_str()) {
                Some(value) => return Ok(value),
                None => return Err(LispError::new(&format!("unbound symbol: {}", name))),
            }
        }
        Sexp::List(items) => {
            if items.len() == 0 { return Ok(Sexp::List(Vec::new())); }
            let head = items[0].clone();
            match &head {
                Sexp::Sym(name) => {
                    let special = name.as_str();
                    if text_equals(special, "quote") {
                        if items.len() != 2 { return Err(LispError::new("quote expects one argument")); }
                        return Ok(items[1].clone());
                    }
                    if text_equals(special, "if") {
                        if items.len() < 3 || items.len() > 4 { return Err(LispError::new("if expects two or three arguments")); }
                        let condition = eval(&items[1], env, stats)?;
                        if is_truthy(&condition) { return eval(&items[2], env, stats); }
                        if items.len() == 4 { return eval(&items[3], env, stats); }
                        return Ok(Sexp::Bool(false));
                    }
                    if text_equals(special, "define") {
                        if items.len() != 3 { return Err(LispError::new("define expects a name and a value")); }
                        match &items[1] {
                            Sexp::Sym(binding) => {
                                let value = eval(&items[2], env, stats)?;
                                env_push(env, binding.as_str(), value);
                                return Ok(Sexp::Sym(binding.clone()));
                            }
                            _ => return Err(LispError::new("define expects a symbol name")),
                        }
                    }
                    if text_equals(special, "set!") {
                        if items.len() != 3 { return Err(LispError::new("set! expects a name and a value")); }
                        match &items[1] {
                            Sexp::Sym(binding) => {
                                let value = eval(&items[2], env, stats)?;
                                if env_set(env, binding.as_str(), value) == false {
                                    return Err(LispError::new(&format!("unbound symbol: {}", binding)));
                                }
                                return Ok(Sexp::Sym(binding.clone()));
                            }
                            _ => return Err(LispError::new("set! expects a symbol name")),
                        }
                    }
                    if text_equals(special, "let") {
                        if items.len() < 3 { return Err(LispError::new("let expects bindings and a body")); }
                        let mut nested = env.clone();
                        match &items[1] {
                            Sexp::List(bindings) => {
                                let mut index: usize = 0;
                                while index < bindings.len() {
                                    match &bindings[index] {
                                        Sexp::List(pair) => {
                                            if pair.len() != 2 { return Err(LispError::new("let binding must have a name and a value")); }
                                            match &pair[0] {
                                                Sexp::Sym(binding) => {
                                                    let value = eval(&pair[1], env, stats)?;
                                                    env_push(&mut nested, binding.as_str(), value);
                                                }
                                                _ => return Err(LispError::new("let binding name must be a symbol")),
                                            }
                                        }
                                        _ => return Err(LispError::new("let expects a list of bindings")),
                                    }
                                    index += 1;
                                }
                            }
                            _ => return Err(LispError::new("let expects a list of bindings")),
                        }
                        let mut result = Sexp::Bool(false);
                        let mut body_index: usize = 2;
                        while body_index < items.len() {
                            result = eval(&items[body_index], &mut nested, stats)?;
                            body_index += 1;
                        }
                        return Ok(result);
                    }
                    if text_equals(special, "lambda") {
                        if items.len() < 3 { return Err(LispError::new("lambda expects parameters and a body")); }
                        let mut params: Vec<String> = Vec::new();
                        match &items[1] {
                            Sexp::List(names) => {
                                let mut index: usize = 0;
                                while index < names.len() {
                                    match &names[index] {
                                        Sexp::Sym(param) => params.push(param.clone()),
                                        _ => return Err(LispError::new("lambda parameters must be symbols")),
                                    }
                                    index += 1;
                                }
                            }
                            _ => return Err(LispError::new("lambda expects a parameter list")),
                        }
                        let mut body: Vec<Sexp> = Vec::new();
                        let mut index: usize = 2;
                        while index < items.len() {
                            body.push(items[index].clone());
                            index += 1;
                        }
                        return Ok(Sexp::Lambda(params, body, env.clone()));
                    }
                    if text_equals(special, "begin") {
                        let mut result = Sexp::Bool(false);
                        let mut index: usize = 1;
                        while index < items.len() {
                            result = eval(&items[index], env, stats)?;
                            index += 1;
                        }
                        return Ok(result);
                    }
                    if text_equals(special, "print") {
                        let mut index: usize = 1;
                        while index < items.len() {
                            let value = eval(&items[index], env, stats)?;
                            println!("{}", display(&value));
                            stats.prints += 1;
                            index += 1;
                        }
                        return Ok(Sexp::Bool(true));
                    }
                    if text_equals(special, "+") || text_equals(special, "-") || text_equals(special, "*") || text_equals(special, "/") {
                        let args = eval_args(items, 1, env, stats)?;
                        return builtin_arithmetic(special, &args);
                    }
                    if text_equals(special, "=") || text_equals(special, "<") || text_equals(special, ">") || text_equals(special, "<=") || text_equals(special, ">=") {
                        let args = eval_args(items, 1, env, stats)?;
                        return builtin_compare(special, &args);
                    }
                    if text_equals(special, "list") || text_equals(special, "car") || text_equals(special, "cdr") || text_equals(special, "cons")
                        || text_equals(special, "length") || text_equals(special, "null?") || text_equals(special, "number?")
                        || text_equals(special, "string?") || text_equals(special, "symbol?") || text_equals(special, "not") {
                        let args = eval_args(items, 1, env, stats)?;
                        return builtin_list(special, &args);
                    }
                }
                _ => {}
            }
            let callee = eval(&items[0], env, stats)?;
            let args = eval_args(items, 1, env, stats)?;
            let caller = env.clone();
            return apply_lambda(&callee, args, &caller, stats);
        }
    }
}
fn embedded_script() -> String {
    let mut script = String::new();
    script.push_str("; embedded script for lisp.ls\n");
    script.push_str("(define answer 42)\n");
    script.push_str("(print answer)\n");
    script.push_str("(define square (lambda (n) (* n n)))\n");
    script.push_str("(print (square 12))\n");
    script.push_str("(define fact (lambda (n) (if (<= n 1) 1 (* n (fact (- n 1))))))\n");
    script.push_str("(print (fact 6))\n");
    script.push_str("(define make-adder (lambda (n) (lambda (m) (+ n m))))\n");
    script.push_str("(define add7 (make-adder 7))\n");
    script.push_str("(print (add7 35))\n");
    script.push_str("(print (list 1 2 3))\n");
    script.push_str("(print (car (list 1 2 3)))\n");
    script.push_str("(print (cdr (list 1 2 3)))\n");
    script.push_str("(print (cons 0 (list 1 2)))\n");
    script.push_str("(print (length (list \"a\" \"b\" \"c\")))\n");
    script.push_str("(print (if (< 3 5) \"less\" \"more\"))\n");
    script.push_str("(let ((x 5) (y 7)) (print (+ x y)))\n");
    script.push_str("(print (quote (a b c)))\n");
    script.push_str("(print 'sym)\n");
    script.push_str("(set! answer (+ answer 1))\n");
    script.push_str("(print answer)\n");
    script.push_str("(define counter 0)\n");
    script.push_str("(define bump (lambda () (begin (set! counter (+ counter 1)) counter)))\n");
    script.push_str("(print (bump))\n");
    script.push_str("(print (bump))\n");
    script.push_str("(print (number? 12) (string? \"text\") (symbol? 'sym) (null? (list)))\n");
    script.push_str("(print (not #f) (not #t))\n");
    script.push_str("(print \"done\")\n");
    return script;
}
fn count_lines(text: &str) -> usize {
    let lines = text.lines();
    return lines.len();
}
fn run_script(text: &str) -> Result<Stats, LispError> {
    let mut parser = Parser::new(text);
    let expressions = parser.parse_all()?;
    let mut env: Vec<Sexp> = Vec::new();
    let mut stats = Stats::new();
    let mut index: usize = 0;
    while index < expressions.len() {
        let expr = expressions[index].clone();
        let _value = eval(&expr, &mut env, &mut stats)?;
        index += 1;
    }
    return Ok(stats);
}
fn report_parse_error(text: &str) {
    let mut parser = Parser::new(text);
    match parser.parse_all() {
        Ok(_expressions) => println!("parse error demo: surprising success"),
        Err(error) => println!("parse error demo: {}", error.describe()),
    }
}
fn main() {
    println!("lisp: a small Lisp interpreter");
    let script = embedded_script();
    println!("embedded script lines: {}", count_lines(&script));
    match run_script(&script) {
        Ok(stats) => {
            println!("expressions evaluated: {}", stats.expressions);
            println!("values printed: {}", stats.prints);
        }
        Err(error) => println!("script error: {}", error.describe()),
    }
    report_parse_error("(+ 1 2");
    report_parse_error("\"unterminated");
    let args = std::process::args();
    if args.len() > 0 {
        let path = args[0].clone();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                println!("file script lines: {}", count_lines(&text));
                match run_script(&text) {
                    Ok(stats) => {
                        println!("expressions evaluated: {}", stats.expressions);
                        println!("values printed: {}", stats.prints);
                    }
                    Err(error) => println!("file script error: {}", error.describe()),
                }
            }
            Err(error) => println!("file read error: {}", error.message()),
        }
    }
}