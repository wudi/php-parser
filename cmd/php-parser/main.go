package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"strings"

	"github.com/wudi/php-parser/lexer"
	"github.com/wudi/php-parser/parser"
)

const (
	ExitSuccess = 0
	ExitError   = 1
)

// Config 配置结构
type Config struct {
	inputFile    string
	outputFormat string
	showTokens   bool
	showAST      bool
	showErrors   bool
	verbose      bool
}

func main() {
	config := parseFlags()

	// 读取输入
	var input string

	if config.inputFile == "" || config.inputFile == "-" {
		// 从标准输入读取
		data, err := ioutil.ReadAll(os.Stdin)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading from stdin: %v\n", err)
			os.Exit(ExitError)
		}
		input = string(data)
	} else {
		// 从文件读取
		data, err := ioutil.ReadFile(config.inputFile)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading file %s: %v\n", config.inputFile, err)
			os.Exit(ExitError)
		}
		input = string(data)
	}

	if strings.TrimSpace(input) == "" {
		fmt.Fprintf(os.Stderr, "Error: empty input\n")
		os.Exit(ExitError)
	}

	// 执行解析
	parseAndOutput(input, config)
}

func parseFlags() Config {
	var config Config

	flag.StringVar(&config.inputFile, "i", "", "Input PHP file (default: stdin)")
	flag.StringVar(&config.outputFormat, "format", "json", "Output format: json, ast, tokens")
	flag.BoolVar(&config.showTokens, "tokens", false, "Show token stream")
	flag.BoolVar(&config.showAST, "ast", false, "Show AST structure")
	flag.BoolVar(&config.showErrors, "errors", false, "Show only errors")
	flag.BoolVar(&config.verbose, "v", false, "Verbose output")

	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "Usage: %s [options] [input-file]\n", os.Args[0])
		fmt.Fprintf(os.Stderr, "\nPHP Parser - 解析 PHP 代码并生成抽象语法树\n\n")
		fmt.Fprintf(os.Stderr, "Options:\n")
		flag.PrintDefaults()
		fmt.Fprintf(os.Stderr, "\nExamples:\n")
		fmt.Fprintf(os.Stderr, "  echo '<?php echo \"Hello\"; ?>' | %s\n", os.Args[0])
		fmt.Fprintf(os.Stderr, "  %s -i example.php -format json\n", os.Args[0])
		fmt.Fprintf(os.Stderr, "  %s -tokens -ast example.php\n", os.Args[0])
	}

	flag.Parse()

	// 处理位置参数
	if flag.NArg() > 0 && config.inputFile == "" {
		config.inputFile = flag.Arg(0)
	}

	return config
}

func parseAndOutput(input string, config Config) {
	// 词法分析
	lex := lexer.New(input)
	var tokens []lexer.Token

	if config.showTokens || config.verbose {
		// 收集所有 tokens
		for {
			token := lex.NextToken()
			tokens = append(tokens, token)
			if token.Type == lexer.T_EOF {
				break
			}
		}

		if config.showTokens {
			fmt.Println("=== TOKENS ===")
			for i, token := range tokens[:len(tokens)-1] { // 排除 EOF
				fmt.Printf("%3d: %-25s %q\n", i+1, lexer.TokenNames[token.Type], token.Value)
			}
			fmt.Println()
		}

		// 重新创建 lexer 用于解析
		lex = lexer.New(input)
	}

	// 语法分析
	p := parser.New(lex)
	program := p.Parse()

	// 收集错误
	parserErrors := p.Errors()
	lexerErrors := lex.GetErrors()

	// 如果只显示错误
	if config.showErrors {
		hasErrors := false

		if len(lexerErrors) > 0 {
			fmt.Println("=== LEXICAL ERRORS ===")
			for i, err := range lexerErrors {
				fmt.Printf("Error %d: %s\n", i+1, err)
			}
			hasErrors = true
		}

		if len(parserErrors) > 0 {
			fmt.Println("=== PARSER ERRORS ===")
			for i, err := range parserErrors {
				fmt.Printf("Error %d: %s\n", i+1, err)
			}
			hasErrors = true
		}

		if !hasErrors {
			fmt.Println("No errors found.")
		}

		os.Exit(ExitSuccess)
		return
	}

	// 显示错误（如果有的话）
	if len(lexerErrors) > 0 || len(parserErrors) > 0 {
		fmt.Fprintf(os.Stderr, "=== ERRORS ===\n")

		if len(lexerErrors) > 0 {
			fmt.Fprintf(os.Stderr, "Lexical Errors:\n")
			for i, err := range lexerErrors {
				fmt.Fprintf(os.Stderr, "  %d: %s\n", i+1, err)
			}
		}

		if len(parserErrors) > 0 {
			fmt.Fprintf(os.Stderr, "Parser Errors:\n")
			for i, err := range parserErrors {
				fmt.Fprintf(os.Stderr, "  %d: %s\n", i+1, err)
			}
		}

		fmt.Fprintf(os.Stderr, "\n")
	}

	// 显示 AST 结构
	if config.showAST || config.verbose {
		fmt.Println("=== AST STRUCTURE ===")
		children := program.GetChildren()
		fmt.Printf("Program with %d statements:\n", len(children))
		for i, stmt := range children {
			fmt.Printf("  %d: %T\n", i+1, stmt)
		}
		fmt.Println()
	}

	// 根据格式输出结果
	switch config.outputFormat {
	case "json":
		outputJSON(program)
	case "ast":
		outputAST(program)
	case "tokens":
		if len(tokens) == 0 {
			// 如果之前没有收集 tokens，现在收集
			lex2 := lexer.New(input)
			for {
				token := lex2.NextToken()
				if token.Type == lexer.T_EOF {
					break
				}
				tokens = append(tokens, token)
			}
		}
		outputTokens(tokens[:len(tokens)-1]) // 排除 EOF
	default:
		fmt.Fprintf(os.Stderr, "Unknown output format: %s\n", config.outputFormat)
		os.Exit(ExitError)
	}

	// 如果有错误，以非零状态退出
	if len(lexerErrors) > 0 || len(parserErrors) > 0 {
		os.Exit(ExitError)
	}
}

func outputJSON(program interface{}) {
	data, err := json.MarshalIndent(program, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error marshaling JSON: %v\n", err)
		os.Exit(ExitError)
	}
	fmt.Println(string(data))
}

func outputAST(program interface{}) {
	if p, ok := program.(fmt.Stringer); ok {
		fmt.Println(p.String())
	} else {
		fmt.Printf("%+v\n", program)
	}
}

func outputTokens(tokens []lexer.Token) {
	for i, token := range tokens {
		fmt.Printf("%3d: %-25s %q at %d:%d\n",
			i+1,
			lexer.TokenNames[token.Type],
			token.Value,
			token.Position.Line,
			token.Position.Column)
	}
}
