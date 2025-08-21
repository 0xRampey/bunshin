#!/usr/bin/env python3
"""
Bunshin Agent Wrapper - A simple agent process for testing and demonstration.

This script acts as a mock agent process that can be spawned by Bunshin.
It reads from stdin, processes commands, and writes responses to stdout.
"""

import os
import sys
import time
import json
import argparse
from datetime import datetime


class BunshinAgent:
    def __init__(self, model, agent_id=None, agent_name=None, project=None, task=None):
        self.model = model
        self.agent_id = agent_id or os.environ.get('BUNSHIN_AGENT_ID', 'unknown')
        self.agent_name = agent_name or os.environ.get('BUNSHIN_AGENT_NAME', 'agent')
        self.project = project or os.environ.get('BUNSHIN_PROJECT')
        self.task = task or os.environ.get('BUNSHIN_TASK')
        self.session_id = os.environ.get('BUNSHIN_SESSION_ID')
        self.window_id = os.environ.get('BUNSHIN_WINDOW_ID')
        self.start_time = time.time()
        self.command_count = 0
        
        # Print startup information
        self.log(f"🚀 Bunshin Agent {self.agent_name} ({self.agent_id}) starting")
        self.log(f"   Model: {self.model}")
        self.log(f"   Session: {self.session_id}")
        self.log(f"   Window: {self.window_id}")
        if self.project:
            self.log(f"   Project: {self.project}")
        if self.task:
            self.log(f"   Task: {self.task}")
        self.log(f"   Working Directory: {os.getcwd()}")
        self.log("")
        self.log("Type 'help' for available commands, 'quit' to exit.")
        
    def log(self, message, level="INFO"):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{timestamp}] [{level}] {message}", flush=True)
        
    def log_error(self, message):
        self.log(message, "ERROR")
        # Also send to stderr for the process manager to capture
        print(f"ERROR: {message}", file=sys.stderr, flush=True)
        
    def process_command(self, command):
        """Process a command and return a response."""
        self.command_count += 1
        command = command.strip()
        
        if not command:
            return
            
        self.log(f"📨 Received command #{self.command_count}: {command}")
        
        if command.lower() == 'quit' or command.lower() == 'exit':
            self.log("👋 Received quit command, shutting down...")
            return False
            
        elif command.lower() == 'help':
            self.show_help()
            
        elif command.lower() == 'status':
            self.show_status()
            
        elif command.lower() == 'ping':
            self.log(f"🏓 Pong! Agent {self.agent_name} is alive")
            
        elif command.startswith('echo '):
            message = command[5:]
            self.log(f"🔊 Echo: {message}")
            
        elif command.startswith('simulate '):
            task = command[9:]
            self.simulate_task(task)
            
        elif command.lower() == 'error':
            self.log_error("This is a simulated error for testing purposes")
            
        elif command.startswith('sleep '):
            try:
                seconds = float(command[6:])
                self.log(f"😴 Sleeping for {seconds} seconds...")
                time.sleep(seconds)
                self.log(f"⏰ Awake after {seconds} seconds")
            except ValueError:
                self.log_error("Invalid sleep duration. Usage: sleep <seconds>")
                
        elif command.lower().startswith('env'):
            if command == 'env':
                self.show_environment()
            elif command.startswith('env '):
                var_name = command[4:]
                value = os.environ.get(var_name, "<not set>")
                self.log(f"Environment variable {var_name}: {value}")
                
        else:
            # Simulate AI model response based on the model type
            self.simulate_ai_response(command)
            
        return True
        
    def show_help(self):
        self.log("📖 Available commands:")
        self.log("  help        - Show this help message")
        self.log("  status      - Show agent status")
        self.log("  ping        - Test agent responsiveness")
        self.log("  echo <text> - Echo back the text")
        self.log("  simulate <task> - Simulate working on a task")
        self.log("  sleep <seconds> - Sleep for specified seconds")
        self.log("  error       - Generate a test error")
        self.log("  env [var]   - Show environment variables")
        self.log("  quit/exit   - Shutdown the agent")
        self.log("  <anything else> - Simulate AI model response")
        
    def show_status(self):
        uptime = time.time() - self.start_time
        hours = int(uptime // 3600)
        minutes = int((uptime % 3600) // 60)
        seconds = int(uptime % 60)
        
        self.log("📊 Agent Status:")
        self.log(f"  Agent ID: {self.agent_id}")
        self.log(f"  Name: {self.agent_name}")
        self.log(f"  Model: {self.model}")
        self.log(f"  Uptime: {hours:02d}:{minutes:02d}:{seconds:02d}")
        self.log(f"  Commands processed: {self.command_count}")
        self.log(f"  Process ID: {os.getpid()}")
        
    def show_environment(self):
        self.log("🌍 Bunshin Environment Variables:")
        bunshin_vars = {k: v for k, v in os.environ.items() if k.startswith('BUNSHIN_')}
        if bunshin_vars:
            for key, value in sorted(bunshin_vars.items()):
                self.log(f"  {key}: {value}")
        else:
            self.log("  No BUNSHIN_* environment variables found")
            
    def simulate_task(self, task):
        self.log(f"🎯 Starting to work on task: {task}")
        
        # Simulate some work with progress updates
        steps = ["Analyzing requirements", "Planning approach", "Implementing solution", "Testing", "Finalizing"]
        for i, step in enumerate(steps, 1):
            self.log(f"  [{i}/{len(steps)}] {step}...")
            time.sleep(0.5)  # Brief delay to simulate work
            
        self.log(f"✅ Task completed: {task}")
        
    def simulate_ai_response(self, prompt):
        # Simulate different AI model behaviors
        model_lower = self.model.lower()
        
        if 'claude' in model_lower:
            self.log(f"🧠 Claude-style response to: '{prompt}'")
            self.log("I'm Claude, an AI assistant. I'd be happy to help you with that task.")
            self.log("Let me think through this step by step...")
            
        elif 'gpt' in model_lower:
            self.log(f"🤖 GPT-style response to: '{prompt}'")
            self.log("As an AI language model, I can assist you with various tasks.")
            self.log("Here's my response to your request...")
            
        else:
            self.log(f"🔮 AI response to: '{prompt}'")
            self.log("Processing your request using advanced AI capabilities...")
            
        # Add some simulated "thinking" time
        time.sleep(0.2)
        self.log("Response complete. How else can I help?")
        
    def run(self):
        """Main agent loop - read from stdin and process commands."""
        self.log(f"🎧 Agent listening for commands...")
        
        try:
            for line in sys.stdin:
                if not self.process_command(line):
                    break
                    
        except KeyboardInterrupt:
            self.log("🛑 Received interrupt signal, shutting down gracefully...")
        except EOFError:
            self.log("📭 Input stream closed, shutting down...")
        except Exception as e:
            self.log_error(f"Unexpected error: {e}")
            return 1
            
        uptime = time.time() - self.start_time
        self.log(f"🏁 Agent {self.agent_name} shutting down after {uptime:.1f} seconds")
        self.log(f"   Processed {self.command_count} commands")
        return 0


def main():
    parser = argparse.ArgumentParser(description="Bunshin Agent Process")
    parser.add_argument('--model', required=True, help='AI model to simulate')
    parser.add_argument('--agent-id', help='Agent ID (overrides environment)')
    parser.add_argument('--agent-name', help='Agent name (overrides environment)')
    parser.add_argument('--project', help='Project name (overrides environment)')
    parser.add_argument('--task', help='Task description (overrides environment)')
    
    args = parser.parse_args()
    
    agent = BunshinAgent(
        model=args.model,
        agent_id=args.agent_id,
        agent_name=args.agent_name,
        project=args.project,
        task=args.task
    )
    
    return agent.run()


if __name__ == '__main__':
    sys.exit(main())