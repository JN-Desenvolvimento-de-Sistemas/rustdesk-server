#define InstallerVersion "1.0.0"
[Setup]
AppId=JN-RustDesk-Configurator
AppName=RustDesk — Configuração JN
AppVersion={#InstallerVersion}
AppPublisher=JN Desenvolvimento de Sistemas
AppPublisherURL=https://github.com/JN-Desenvolvimento-de-Sistemas/rustdesk-server
DefaultDirName={autopf}\RustDesk
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=no
DisableWelcomePage=no
OutputDir=output
OutputBaseFilename=Instalar-RustDesk-JN
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
[Messages]
WelcomeLabel2=Este assistente instala o RustDesk oficial e configura o acesso aos servidores da JN.%n%nÉ necessária uma conexão com a internet. Se já houver uma instalação, suas preferências e identidade serão preservadas. As configurações de servidor serão atualizadas.
FinishedHeadingLabel=RustDesk configurado
FinishedLabel=O RustDesk está instalado e configurado.%n%nPara iniciar atendimentos, entre no aplicativo com sua própria conta. O computador que recebe suporte pode permanecer sem login.
[Files]
Source: "install.ps1"; Flags: dontcopy
Source: "RustDeskInstaller.psm1"; Flags: dontcopy
Source: "client.json"; Flags: dontcopy
[Code]
var
  InstalledPath: String;
  ConfigurationDone: Boolean;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ExitCode: Integer;
  Params: String;
  Lines: TArrayOfString;
begin
  Result := '';
  if ConfigurationDone then exit;
  ExtractTemporaryFile('install.ps1');
  ExtractTemporaryFile('RustDeskInstaller.psm1');
  ExtractTemporaryFile('client.json');
  DeleteFile(ExpandConstant('{tmp}\error.txt'));
  WizardForm.PreparingLabel.Caption := 'Instalando e configurando o RustDesk. Aguarde...';
  Params := '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ExpandConstant('{tmp}\install.ps1') + '" -ResultDirectory "' + ExpandConstant('{tmp}') + '"';
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Params, '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then begin
    Result := 'Não foi possível iniciar a instalação. Verifique as permissões do Windows.';
    exit;
  end;
  if ExitCode <> 0 then begin
    if LoadStringsFromFile(ExpandConstant('{tmp}\error.txt'), Lines) and (GetArrayLength(Lines) > 0) then
      Result := Trim(Lines[0])
    else
      Result := 'Não foi possível concluir a configuração. Tente novamente.';
    exit;
  end;
  if not LoadStringsFromFile(ExpandConstant('{tmp}\installed-path.txt'), Lines) or (GetArrayLength(Lines) = 0) then begin
    Result := 'Não foi possível confirmar o caminho do RustDesk.';
    exit;
  end;
  InstalledPath := Trim(Lines[0]);
  ConfigurationDone := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExitCode: Integer;
begin
  if (CurStep = ssDone) and ConfigurationDone and not WizardSilent then
    ExecAsOriginalUser(InstalledPath, '', '', SW_SHOWNORMAL, ewNoWait, ExitCode);
end;
